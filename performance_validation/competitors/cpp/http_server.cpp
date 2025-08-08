// C++ HTTP server benchmark implementation for fair comparison with Seen
// High-performance HTTP server using similar architecture to Seen implementation

#include <iostream>
#include <string>
#include <vector>
#include <unordered_map>
#include <chrono>
#include <thread>
#include <mutex>
#include <atomic>
#include <sstream>
#include <fstream>
#include <filesystem>
#include <algorithm>
#include <queue>
#include <condition_variable>
#include <functional>
#include <cctype>

#ifdef _WIN32
    #include <winsock2.h>
    #include <ws2tcpip.h>
    #pragma comment(lib, "ws2_32.lib")
    #define close closesocket
    typedef int socklen_t;
#else
    #include <sys/socket.h>
    #include <netinet/in.h>
    #include <arpa/inet.h>
    #include <unistd.h>
#endif

// HTTP Request representation
class HttpRequest {
public:
    std::string method;
    std::string path;
    std::string version;
    std::unordered_map<std::string, std::string> headers;
    std::string body;

    HttpRequest() = default;

    static std::optional<HttpRequest> parseFromBuffer(const std::string& buffer) {
        std::istringstream stream(buffer);
        std::string line;
        std::vector<std::string> lines;
        
        while (std::getline(stream, line)) {
            // Remove carriage return if present
            if (!line.empty() && line.back() == '\r') {
                line.pop_back();
            }
            lines.push_back(line);
        }
        
        if (lines.empty()) {
            return std::nullopt;
        }

        HttpRequest request;

        // Parse request line (GET /path HTTP/1.1)
        std::istringstream requestLine(lines[0]);
        std::string method, path, version;
        if (!(requestLine >> method >> path >> version)) {
            return std::nullopt;
        }

        request.method = method;
        request.path = path;
        request.version = version;

        // Parse headers
        size_t i = 1;
        while (i < lines.size() && !lines[i].empty()) {
            size_t colonPos = lines[i].find(':');
            if (colonPos != std::string::npos) {
                std::string key = lines[i].substr(0, colonPos);
                std::string value = lines[i].substr(colonPos + 1);
                
                // Trim whitespace
                key.erase(key.find_last_not_of(" \t") + 1);
                key.erase(0, key.find_first_not_of(" \t"));
                value.erase(value.find_last_not_of(" \t") + 1);
                value.erase(0, value.find_first_not_of(" \t"));
                
                // Convert key to lowercase
                std::transform(key.begin(), key.end(), key.begin(), ::tolower);
                request.headers[key] = value;
            }
            i++;
        }

        // Parse body (if present)
        i++; // Skip empty line
        if (i < lines.size()) {
            std::ostringstream bodyStream;
            for (size_t j = i; j < lines.size(); ++j) {
                if (j > i) bodyStream << "\n";
                bodyStream << lines[j];
            }
            request.body = bodyStream.str();
        }

        return request;
    }

    std::optional<std::string> getHeader(const std::string& name) const {
        std::string lowerName = name;
        std::transform(lowerName.begin(), lowerName.end(), lowerName.begin(), ::tolower);
        auto it = headers.find(lowerName);
        if (it != headers.end()) {
            return it->second;
        }
        return std::nullopt;
    }
};

// HTTP Response representation
class HttpResponse {
public:
    std::string version = "HTTP/1.1";
    uint16_t statusCode;
    std::string statusText;
    std::unordered_map<std::string, std::string> headers;
    std::string body;

    HttpResponse(uint16_t code, const std::string& text) : statusCode(code), statusText(text) {}

    static HttpResponse ok(const std::string& body) {
        HttpResponse response(200, "OK");
        response.setBody(body);
        return response;
    }

    static HttpResponse notFound() {
        HttpResponse response(404, "Not Found");
        response.setBody("<html><body><h1>404 Not Found</h1></body></html>");
        response.setHeader("Content-Type", "text/html");
        return response;
    }

    static HttpResponse internalServerError() {
        HttpResponse response(500, "Internal Server Error");
        response.setBody("<html><body><h1>500 Internal Server Error</h1></body></html>");
        response.setHeader("Content-Type", "text/html");
        return response;
    }

    static HttpResponse badRequest() {
        HttpResponse response(400, "Bad Request");
        response.setBody("<html><body><h1>400 Bad Request</h1></body></html>");
        response.setHeader("Content-Type", "text/html");
        return response;
    }

    void setHeader(const std::string& key, const std::string& value) {
        headers[key] = value;
    }

    void setBody(const std::string& bodyContent) {
        body = bodyContent;
        setHeader("Content-Length", std::to_string(body.length()));
    }

    std::string toBytes() const {
        std::ostringstream response;
        response << version << " " << statusCode << " " << statusText << "\r\n";

        for (const auto& [key, value] : headers) {
            response << key << ": " << value << "\r\n";
        }

        response << "\r\n";
        response << body;

        return response.str();
    }
};

// Thread pool for handling concurrent connections
class ThreadPool {
public:
    ThreadPool(size_t numThreads) : stop(false) {
        for (size_t i = 0; i < numThreads; ++i) {
            workers.emplace_back([this] {
                while (true) {
                    std::function<void()> task;

                    {
                        std::unique_lock<std::mutex> lock(queueMutex);
                        condition.wait(lock, [this] { return stop || !tasks.empty(); });
                        
                        if (stop && tasks.empty()) {
                            return;
                        }

                        task = std::move(tasks.front());
                        tasks.pop();
                    }

                    task();
                }
            });
        }
    }

    template<class F>
    void enqueue(F&& f) {
        {
            std::unique_lock<std::mutex> lock(queueMutex);
            if (stop) {
                return;
            }
            tasks.emplace(std::forward<F>(f));
        }
        condition.notify_one();
    }

    ~ThreadPool() {
        {
            std::unique_lock<std::mutex> lock(queueMutex);
            stop = true;
        }
        condition.notify_all();
        for (std::thread& worker : workers) {
            if (worker.joinable()) {
                worker.join();
            }
        }
    }

private:
    std::vector<std::thread> workers;
    std::queue<std::function<void()>> tasks;
    std::mutex queueMutex;
    std::condition_variable condition;
    bool stop;
};

// Route handler type
using RouteHandler = std::function<HttpResponse(const HttpRequest&)>;

// Simple HTTP Server implementation
class HttpServer {
public:
    HttpServer(uint16_t port) : running(false), requestCount(0), responseTimeSum(0) {
        initializeSocket();
        setupServer(port);
    }

    ~HttpServer() {
        stop();
        cleanup();
    }

    void addRoute(const std::string& path, RouteHandler handler) {
        routes[path] = handler;
    }

    HttpResponse handleRequest(const HttpRequest& request) {
        auto startTime = std::chrono::high_resolution_clock::now();

        HttpResponse response = [&]() -> HttpResponse {
            auto routeIt = routes.find(request.path);
            if (routeIt != routes.end()) {
                return routeIt->second(request);
            }

            if (request.path == "/") {
                return HttpResponse::ok("Hello, World!");
            } else if (request.path == "/health") {
                auto response = HttpResponse::ok("OK");
                response.setHeader("Content-Type", "text/plain");
                return response;
            } else if (request.path == "/stats") {
                uint64_t reqCount = requestCount.load();
                uint64_t avgResponseTime = reqCount > 0 ? responseTimeSum.load() / reqCount : 0;

                std::ostringstream stats;
                stats << R"({"requests": )" << reqCount << R"(, "avgResponseTimeUs": )" << avgResponseTime << "}";

                auto response = HttpResponse::ok(stats.str());
                response.setHeader("Content-Type", "application/json");
                return response;
            } else if (request.path == "/echo") {
                auto response = HttpResponse::ok(request.body);
                response.setHeader("Content-Type", "text/plain");
                return response;
            } else if (request.path.starts_with("/static/")) {
                std::string fileName = request.path.substr(8); // Remove "/static/" prefix
                return serveStaticFile(fileName);
            }

            return HttpResponse::notFound();
        }();

        auto endTime = std::chrono::high_resolution_clock::now();
        auto responseTime = std::chrono::duration_cast<std::chrono::microseconds>(endTime - startTime).count();

        requestCount.fetch_add(1);
        responseTimeSum.fetch_add(responseTime);

        return response;
    }

    void start() {
        running = true;
        std::cout << "HTTP Server started on 127.0.0.1:" << serverPort << std::endl;

        ThreadPool threadPool(4); // 4 worker threads

        while (running) {
            sockaddr_in clientAddr{};
            socklen_t clientLen = sizeof(clientAddr);

#ifdef _WIN32
            SOCKET clientSocket = accept(serverSocket, reinterpret_cast<sockaddr*>(&clientAddr), &clientLen);
            if (clientSocket == INVALID_SOCKET) {
#else
            int clientSocket = accept(serverSocket, reinterpret_cast<sockaddr*>(&clientAddr), &clientLen);
            if (clientSocket < 0) {
#endif
                if (running) {
                    std::cerr << "Error accepting connection" << std::endl;
                }
                continue;
            }

            threadPool.enqueue([this, clientSocket] {
                handleConnection(clientSocket);
            });
        }
    }

    void stop() {
        running = false;
    }

    std::pair<uint64_t, double> getStats() const {
        uint64_t reqCount = requestCount.load();
        double avgResponseTime = reqCount > 0 ? 
            static_cast<double>(responseTimeSum.load()) / static_cast<double>(reqCount) : 0.0;
        return {reqCount, avgResponseTime};
    }

    std::atomic<bool>& getRunningFlag() { return running; }
    std::atomic<uint64_t>& getRequestCount() { return requestCount; }
    std::atomic<uint64_t>& getResponseTimeSum() { return responseTimeSum; }

private:
#ifdef _WIN32
    SOCKET serverSocket;
#else
    int serverSocket;
#endif
    uint16_t serverPort;
    std::atomic<bool> running;
    std::atomic<uint64_t> requestCount;
    std::atomic<uint64_t> responseTimeSum; // in microseconds
    std::unordered_map<std::string, RouteHandler> routes;

    void initializeSocket() {
#ifdef _WIN32
        WSADATA wsaData;
        if (WSAStartup(MAKEWORD(2, 2), &wsaData) != 0) {
            throw std::runtime_error("WSAStartup failed");
        }
#endif
    }

    void setupServer(uint16_t port) {
        serverPort = port;

#ifdef _WIN32
        serverSocket = socket(AF_INET, SOCK_STREAM, 0);
        if (serverSocket == INVALID_SOCKET) {
#else
        serverSocket = socket(AF_INET, SOCK_STREAM, 0);
        if (serverSocket < 0) {
#endif
            throw std::runtime_error("Failed to create socket");
        }

        int opt = 1;
#ifdef _WIN32
        if (setsockopt(serverSocket, SOL_SOCKET, SO_REUSEADDR, reinterpret_cast<char*>(&opt), sizeof(opt)) < 0) {
#else
        if (setsockopt(serverSocket, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt)) < 0) {
#endif
            throw std::runtime_error("Failed to set socket options");
        }

        sockaddr_in serverAddr{};
        serverAddr.sin_family = AF_INET;
        serverAddr.sin_addr.s_addr = INADDR_ANY;
        serverAddr.sin_port = htons(port);

        if (bind(serverSocket, reinterpret_cast<sockaddr*>(&serverAddr), sizeof(serverAddr)) < 0) {
            throw std::runtime_error("Failed to bind socket");
        }

        if (listen(serverSocket, 10) < 0) {
            throw std::runtime_error("Failed to listen on socket");
        }
    }

    HttpResponse serveStaticFile(const std::string& fileName) {
        // Security: basic path traversal protection
        if (fileName.find("..") != std::string::npos || fileName.find("/") != std::string::npos) {
            return HttpResponse::notFound();
        }

        std::string filePath = "static/" + fileName;

        std::ifstream file(filePath);
        if (!file.is_open()) {
            return HttpResponse::notFound();
        }

        std::ostringstream content;
        content << file.rdbuf();

        auto response = HttpResponse::ok(content.str());

        // Set content type based on file extension
        size_t dotPos = fileName.find_last_of('.');
        if (dotPos != std::string::npos) {
            std::string extension = fileName.substr(dotPos + 1);
            std::string contentType = "application/octet-stream";

            if (extension == "html") contentType = "text/html";
            else if (extension == "css") contentType = "text/css";
            else if (extension == "js") contentType = "application/javascript";
            else if (extension == "json") contentType = "application/json";
            else if (extension == "txt") contentType = "text/plain";

            response.setHeader("Content-Type", contentType);
        }

        return response;
    }

#ifdef _WIN32
    void handleConnection(SOCKET clientSocket) {
#else
    void handleConnection(int clientSocket) {
#endif
        char buffer[4096];
        int bytesRead = recv(clientSocket, buffer, sizeof(buffer) - 1, 0);

        if (bytesRead > 0) {
            buffer[bytesRead] = '\0';
            std::string requestData(buffer);

            auto maybeRequest = HttpRequest::parseFromBuffer(requestData);
            HttpResponse response = maybeRequest ? 
                handleRequest(*maybeRequest) : HttpResponse::badRequest();

            std::string responseStr = response.toBytes();
            send(clientSocket, responseStr.c_str(), responseStr.length(), 0);
        }

        close(clientSocket);
    }

    void cleanup() {
        if (serverSocket >= 0) {
            close(serverSocket);
        }
#ifdef _WIN32
        WSACleanup();
#endif
    }
};

// Benchmark functions
void benchmarkHttpServerThroughput() {
    std::cout << "Running C++ HTTP server throughput benchmark..." << std::endl;

    try {
        // Start server in background thread
        HttpServer server(8080);
        auto& serverRunning = server.getRunningFlag();
        auto& serverRequestCount = server.getRequestCount();
        auto& serverResponseTimeSum = server.getResponseTimeSum();

        std::thread serverThread([&server] {
            try {
                server.start();
            } catch (const std::exception& e) {
                std::cerr << "Server error: " << e.what() << std::endl;
            }
        });

        // Wait for server to start
        std::this_thread::sleep_for(std::chrono::milliseconds(200));

        // Run concurrent load test
        auto startTime = std::chrono::high_resolution_clock::now();
        const int numberOfClients = 50;
        const int requestsPerClient = 100;
        std::vector<std::thread> clientThreads;

        for (int clientId = 0; clientId < numberOfClients; ++clientId) {
            clientThreads.emplace_back([clientId, requestsPerClient] {
                for (int requestId = 0; requestId < requestsPerClient; ++requestId) {
#ifdef _WIN32
                    SOCKET sock = socket(AF_INET, SOCK_STREAM, 0);
                    if (sock == INVALID_SOCKET) break;
#else
                    int sock = socket(AF_INET, SOCK_STREAM, 0);
                    if (sock < 0) break;
#endif

                    sockaddr_in serverAddr{};
                    serverAddr.sin_family = AF_INET;
                    serverAddr.sin_port = htons(8080);
                    inet_pton(AF_INET, "127.0.0.1", &serverAddr.sin_addr);

                    if (connect(sock, reinterpret_cast<sockaddr*>(&serverAddr), sizeof(serverAddr)) == 0) {
                        std::ostringstream request;
                        request << "GET /?client=" << clientId << "&request=" << requestId << " HTTP/1.1\r\n"
                               << "Host: localhost\r\n"
                               << "Connection: close\r\n"
                               << "\r\n";

                        std::string requestStr = request.str();
                        send(sock, requestStr.c_str(), requestStr.length(), 0);

                        char buffer[1024];
                        recv(sock, buffer, sizeof(buffer), 0);
                    }

                    close(sock);
                }
            });
        }

        // Wait for all clients to complete
        for (auto& thread : clientThreads) {
            thread.join();
        }

        auto endTime = std::chrono::high_resolution_clock::now();
        auto elapsed = std::chrono::duration<double>(endTime - startTime);
        int totalRequests = numberOfClients * requestsPerClient;
        double requestsPerSecond = totalRequests / elapsed.count();

        // Stop server
        serverRunning = false;

        // Connect once more to wake up the server
#ifdef _WIN32
        SOCKET wakeSocket = socket(AF_INET, SOCK_STREAM, 0);
        if (wakeSocket != INVALID_SOCKET) {
#else
        int wakeSocket = socket(AF_INET, SOCK_STREAM, 0);
        if (wakeSocket >= 0) {
#endif
            sockaddr_in addr{};
            addr.sin_family = AF_INET;
            addr.sin_port = htons(8080);
            inet_pton(AF_INET, "127.0.0.1", &addr.sin_addr);
            connect(wakeSocket, reinterpret_cast<sockaddr*>(&addr), sizeof(addr));
            close(wakeSocket);
        }

        if (serverThread.joinable()) {
            serverThread.join();
        }

        uint64_t finalRequestCount = serverRequestCount.load();
        double avgResponseTime = finalRequestCount > 0 ? 
            static_cast<double>(serverResponseTimeSum.load()) / static_cast<double>(finalRequestCount) : 0.0;

        std::cout << "C++ HTTP Server Throughput Performance:" << std::endl;
        std::cout << "  Total requests handled: " << finalRequestCount << std::endl;
        std::cout << "  Average response time: " << std::fixed << std::setprecision(2) << avgResponseTime << "μs" << std::endl;
        std::cout << "  Requests per second: " << std::fixed << std::setprecision(0) << requestsPerSecond << std::endl;
        std::cout << "  Total elapsed time: " << std::fixed << std::setprecision(2) << elapsed.count() << "s" << std::endl;

    } catch (const std::exception& e) {
        std::cerr << "Benchmark error: " << e.what() << std::endl;
    }
}

void benchmarkHttpServerLatency() {
    std::cout << "Running C++ HTTP server latency benchmark..." << std::endl;

    try {
        // Test individual request latency
        HttpServer server(8081);
        auto& serverRunning = server.getRunningFlag();

        std::thread serverThread([&server] {
            try {
                server.start();
            } catch (const std::exception& e) {
                std::cerr << "Server error: " << e.what() << std::endl;
            }
        });

        std::this_thread::sleep_for(std::chrono::milliseconds(200));

        std::vector<double> latencies;
        const int iterations = 1000;

        for (int i = 0; i < iterations; ++i) {
#ifdef _WIN32
            SOCKET sock = socket(AF_INET, SOCK_STREAM, 0);
            if (sock == INVALID_SOCKET) continue;
#else
            int sock = socket(AF_INET, SOCK_STREAM, 0);
            if (sock < 0) continue;
#endif

            sockaddr_in serverAddr{};
            serverAddr.sin_family = AF_INET;
            serverAddr.sin_port = htons(8081);
            inet_pton(AF_INET, "127.0.0.1", &serverAddr.sin_addr);

            auto startTime = std::chrono::high_resolution_clock::now();

            if (connect(sock, reinterpret_cast<sockaddr*>(&serverAddr), sizeof(serverAddr)) == 0) {
                const char* request = "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
                send(sock, request, strlen(request), 0);

                char buffer[1024];
                if (recv(sock, buffer, sizeof(buffer), 0) > 0) {
                    auto endTime = std::chrono::high_resolution_clock::now();
                    auto latencyUs = std::chrono::duration<double, std::micro>(endTime - startTime).count();
                    latencies.push_back(latencyUs);
                }
            }

            close(sock);
        }

        serverRunning = false;

        // Connect once more to wake up the server
#ifdef _WIN32
        SOCKET wakeSocket = socket(AF_INET, SOCK_STREAM, 0);
        if (wakeSocket != INVALID_SOCKET) {
#else
        int wakeSocket = socket(AF_INET, SOCK_STREAM, 0);
        if (wakeSocket >= 0) {
#endif
            sockaddr_in addr{};
            addr.sin_family = AF_INET;
            addr.sin_port = htons(8081);
            inet_pton(AF_INET, "127.0.0.1", &addr.sin_addr);
            connect(wakeSocket, reinterpret_cast<sockaddr*>(&addr), sizeof(addr));
            close(wakeSocket);
        }

        if (serverThread.joinable()) {
            serverThread.join();
        }

        if (!latencies.empty()) {
            std::sort(latencies.begin(), latencies.end());

            double avgLatency = std::accumulate(latencies.begin(), latencies.end(), 0.0) / latencies.size();
            size_t p95Index = std::min(static_cast<size_t>(latencies.size() * 0.95), latencies.size() - 1);
            size_t p99Index = std::min(static_cast<size_t>(latencies.size() * 0.99), latencies.size() - 1);
            double p95Latency = latencies[p95Index];
            double p99Latency = latencies[p99Index];

            std::cout << "C++ HTTP Server Latency Performance:" << std::endl;
            std::cout << "  Average latency: " << std::fixed << std::setprecision(2) << avgLatency << "μs" << std::endl;
            std::cout << "  95th percentile: " << std::fixed << std::setprecision(2) << p95Latency << "μs" << std::endl;
            std::cout << "  99th percentile: " << std::fixed << std::setprecision(2) << p99Latency << "μs" << std::endl;
            std::cout << "  Total requests: " << latencies.size() << std::endl;
        }

    } catch (const std::exception& e) {
        std::cerr << "Benchmark error: " << e.what() << std::endl;
    }
}

int main() {
    std::cout << "Running C++ HTTP Server Benchmarks..." << std::endl;

    try {
        benchmarkHttpServerThroughput();
        benchmarkHttpServerLatency();

        std::cout << "C++ HTTP server benchmarks completed successfully!" << std::endl;
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "Error running C++ HTTP server benchmark: " << e.what() << std::endl;
        return 1;
    }
}