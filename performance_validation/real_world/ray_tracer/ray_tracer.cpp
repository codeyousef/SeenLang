// Ray Tracer Benchmark - C++ Implementation
#include <iostream>
#include <vector>
#include <cmath>
#include <chrono>
#include <fstream>
#include <algorithm>
#include <random>

struct Vec3 {
    double x, y, z;
    
    Vec3(double x = 0, double y = 0, double z = 0) : x(x), y(y), z(z) {}
    
    Vec3 operator+(const Vec3& v) const { return Vec3(x + v.x, y + v.y, z + v.z); }
    Vec3 operator-(const Vec3& v) const { return Vec3(x - v.x, y - v.y, z - v.z); }
    Vec3 operator*(double t) const { return Vec3(x * t, y * t, z * t); }
    Vec3 operator/(double t) const { return Vec3(x / t, y / t, z / t); }
    
    double dot(const Vec3& v) const { return x * v.x + y * v.y + z * v.z; }
    Vec3 cross(const Vec3& v) const {
        return Vec3(y * v.z - z * v.y, z * v.x - x * v.z, x * v.y - y * v.x);
    }
    
    double length() const { return std::sqrt(x * x + y * y + z * z); }
    Vec3 normalized() const { double l = length(); return Vec3(x / l, y / l, z / l); }
};

struct Ray {
    Vec3 origin;
    Vec3 direction;
    
    Ray(const Vec3& o, const Vec3& d) : origin(o), direction(d.normalized()) {}
    Vec3 at(double t) const { return origin + direction * t; }
};

struct Sphere {
    Vec3 center;
    double radius;
    Vec3 color;
    double reflectivity;
    
    Sphere(const Vec3& c, double r, const Vec3& col, double ref = 0)
        : center(c), radius(r), color(col), reflectivity(ref) {}
    
    bool intersect(const Ray& ray, double& t) const {
        Vec3 oc = ray.origin - center;
        double a = ray.direction.dot(ray.direction);
        double b = 2.0 * oc.dot(ray.direction);
        double c = oc.dot(oc) - radius * radius;
        double discriminant = b * b - 4 * a * c;
        
        if (discriminant < 0) return false;
        
        double t1 = (-b - std::sqrt(discriminant)) / (2 * a);
        double t2 = (-b + std::sqrt(discriminant)) / (2 * a);
        
        if (t1 > 0.001) {
            t = t1;
            return true;
        }
        if (t2 > 0.001) {
            t = t2;
            return true;
        }
        return false;
    }
    
    Vec3 normal(const Vec3& point) const {
        return (point - center).normalized();
    }
};

class Scene {
    std::vector<Sphere> spheres;
    Vec3 light_pos;
    Vec3 ambient_light;
    
public:
    Scene() : light_pos(5, 10, 5), ambient_light(0.1, 0.1, 0.1) {
        // Create a scene with multiple spheres
        spheres.push_back(Sphere(Vec3(0, 0, -5), 1.0, Vec3(1, 0, 0), 0.3));      // Red
        spheres.push_back(Sphere(Vec3(2, 0, -6), 1.0, Vec3(0, 1, 0), 0.5));      // Green
        spheres.push_back(Sphere(Vec3(-2, 0, -4), 0.8, Vec3(0, 0, 1), 0.7));     // Blue
        spheres.push_back(Sphere(Vec3(0, -101, -5), 100, Vec3(0.8, 0.8, 0.8))); // Floor
        spheres.push_back(Sphere(Vec3(1, 1, -3), 0.5, Vec3(1, 1, 0), 0.2));      // Yellow
    }
    
    Vec3 trace(const Ray& ray, int depth = 0) {
        if (depth > 5) return Vec3(0, 0, 0);
        
        double min_t = 1e10;
        const Sphere* hit_sphere = nullptr;
        
        // Find closest intersection
        for (const auto& sphere : spheres) {
            double t;
            if (sphere.intersect(ray, t) && t < min_t) {
                min_t = t;
                hit_sphere = &sphere;
            }
        }
        
        if (!hit_sphere) {
            // Sky gradient
            double y = ray.direction.y;
            return Vec3(0.5, 0.7, 1.0) * (1 - y) + Vec3(1, 1, 1) * y;
        }
        
        Vec3 hit_point = ray.at(min_t);
        Vec3 normal = hit_sphere->normal(hit_point);
        
        // Basic lighting
        Vec3 to_light = (light_pos - hit_point).normalized();
        double diffuse = std::max(0.0, normal.dot(to_light));
        
        // Check shadow
        Ray shadow_ray(hit_point, to_light);
        bool in_shadow = false;
        for (const auto& sphere : spheres) {
            double t;
            if (&sphere != hit_sphere && sphere.intersect(shadow_ray, t)) {
                in_shadow = true;
                break;
            }
        }
        
        Vec3 color = hit_sphere->color * ambient_light;
        if (!in_shadow) {
            color = color + hit_sphere->color * diffuse * 0.7;
        }
        
        // Reflection
        if (hit_sphere->reflectivity > 0) {
            Vec3 reflected = ray.direction - normal * (2 * ray.direction.dot(normal));
            Ray reflection_ray(hit_point, reflected);
            Vec3 reflection_color = trace(reflection_ray, depth + 1);
            color = color * (1 - hit_sphere->reflectivity) + reflection_color * hit_sphere->reflectivity;
        }
        
        return color;
    }
    
    void render(int width, int height, std::vector<Vec3>& pixels) {
        double aspect = double(width) / height;
        double fov = 60.0 * M_PI / 180.0;
        double scale = std::tan(fov / 2);
        
        for (int y = 0; y < height; y++) {
            for (int x = 0; x < width; x++) {
                double px = (2 * (x + 0.5) / width - 1) * aspect * scale;
                double py = (1 - 2 * (y + 0.5) / height) * scale;
                
                Ray ray(Vec3(0, 0, 0), Vec3(px, py, -1).normalized());
                pixels[y * width + x] = trace(ray);
            }
        }
    }
};

int main(int argc, char* argv[]) {
    int width = argc > 1 ? std::atoi(argv[1]) : 400;
    int height = argc > 2 ? std::atoi(argv[2]) : 300;
    int iterations = argc > 3 ? std::atoi(argv[3]) : 10;
    
    std::cout << "Ray tracer benchmark: " << width << "x" << height << " for " << iterations << " iterations\n";
    
    Scene scene;
    std::vector<Vec3> pixels(width * height);
    
    // Warmup
    for (int i = 0; i < 2; i++) {
        scene.render(width, height, pixels);
    }
    
    // Benchmark
    std::vector<double> times;
    
    for (int i = 0; i < iterations; i++) {
        auto start = std::chrono::high_resolution_clock::now();
        
        scene.render(width, height, pixels);
        
        auto end = std::chrono::high_resolution_clock::now();
        auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end - start);
        times.push_back(duration.count() / 1000000.0);
    }
    
    // Calculate statistics
    double sum = 0;
    for (double t : times) sum += t;
    double avg = sum / times.size();
    
    double rays_per_sec = (width * height) / avg;
    
    // Output JSON results
    std::cout << "{\n";
    std::cout << "  \"language\": \"cpp\",\n";
    std::cout << "  \"benchmark\": \"ray_tracer\",\n";
    std::cout << "  \"width\": " << width << ",\n";
    std::cout << "  \"height\": " << height << ",\n";
    std::cout << "  \"iterations\": " << iterations << ",\n";
    std::cout << "  \"times\": [";
    for (size_t i = 0; i < times.size(); i++) {
        std::cout << times[i];
        if (i < times.size() - 1) std::cout << ", ";
    }
    std::cout << "],\n";
    std::cout << "  \"average_time\": " << avg << ",\n";
    std::cout << "  \"rays_per_second\": " << rays_per_sec << "\n";
    std::cout << "}\n";
    
    return 0;
}