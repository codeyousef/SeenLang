#include <iostream>
#include <vector>
#include <cmath>
#include <chrono>
#include <cstdlib>

// Simple ray tracer benchmark implementation
struct Vec3 {
    double x, y, z;
    Vec3(double x = 0, double y = 0, double z = 0) : x(x), y(y), z(z) {}
    
    Vec3 operator+(const Vec3& v) const { return Vec3(x + v.x, y + v.y, z + v.z); }
    Vec3 operator-(const Vec3& v) const { return Vec3(x - v.x, y - v.y, z - v.z); }
    Vec3 operator*(double t) const { return Vec3(x * t, y * t, z * t); }
    double dot(const Vec3& v) const { return x * v.x + y * v.y + z * v.z; }
    double length() const { return std::sqrt(x * x + y * y + z * z); }
    Vec3 normalize() const { double l = length(); return Vec3(x/l, y/l, z/l); }
};

struct Ray {
    Vec3 origin, direction;
    Ray(const Vec3& o, const Vec3& d) : origin(o), direction(d.normalize()) {}
};

struct Sphere {
    Vec3 center;
    double radius;
    Vec3 color;
    
    Sphere(const Vec3& c, double r, const Vec3& col) 
        : center(c), radius(r), color(col) {}
    
    bool intersect(const Ray& ray, double& t) const {
        Vec3 oc = ray.origin - center;
        double b = oc.dot(ray.direction);
        double c = oc.dot(oc) - radius * radius;
        double discriminant = b * b - c;
        
        if (discriminant < 0) return false;
        
        double sqrt_d = std::sqrt(discriminant);
        double t1 = -b - sqrt_d;
        double t2 = -b + sqrt_d;
        
        if (t1 > 0) {
            t = t1;
            return true;
        } else if (t2 > 0) {
            t = t2;
            return true;
        }
        return false;
    }
};

class SimpleRayTracer {
private:
    std::vector<Sphere> spheres;
    int width, height;
    
public:
    SimpleRayTracer(int w, int h) : width(w), height(h) {
        // Create simple scene
        spheres.push_back(Sphere(Vec3(0, 0, -5), 1.0, Vec3(1, 0, 0)));
        spheres.push_back(Sphere(Vec3(-2, 0, -6), 1.0, Vec3(0, 1, 0)));
        spheres.push_back(Sphere(Vec3(2, 0, -4), 0.5, Vec3(0, 0, 1)));
        spheres.push_back(Sphere(Vec3(0, -101, -5), 100, Vec3(0.5, 0.5, 0.5)));
    }
    
    Vec3 trace(const Ray& ray) {
        double closest_t = INFINITY;
        const Sphere* hit_sphere = nullptr;
        
        for (const auto& sphere : spheres) {
            double t;
            if (sphere.intersect(ray, t) && t < closest_t) {
                closest_t = t;
                hit_sphere = &sphere;
            }
        }
        
        if (hit_sphere) {
            return hit_sphere->color;
        }
        
        // Sky gradient
        double t = 0.5 * (ray.direction.y + 1.0);
        return Vec3(1, 1, 1) * (1.0 - t) + Vec3(0.5, 0.7, 1.0) * t;
    }
    
    void render() {
        std::vector<Vec3> pixels(width * height);
        
        for (int y = 0; y < height; ++y) {
            for (int x = 0; x < width; ++x) {
                double u = (x + 0.5) / width - 0.5;
                double v = (y + 0.5) / height - 0.5;
                
                Ray ray(Vec3(0, 0, 0), Vec3(u, -v, -1));
                pixels[y * width + x] = trace(ray);
            }
        }
    }
};

int main(int argc, char* argv[]) {
    int width = argc > 1 ? std::atoi(argv[1]) : 200;
    int height = argc > 2 ? std::atoi(argv[2]) : 150;
    int samples = argc > 3 ? std::atoi(argv[3]) : 1;
    
    SimpleRayTracer tracer(width, height);
    
    auto start = std::chrono::high_resolution_clock::now();
    
    for (int s = 0; s < samples; ++s) {
        tracer.render();
    }
    
    auto end = std::chrono::high_resolution_clock::now();
    
    double render_time = std::chrono::duration<double>(end - start).count();
    double pixels_per_sec = (width * height * samples) / render_time;
    size_t memory_usage = sizeof(Sphere) * 4 + width * height * sizeof(Vec3);
    
    // Output in expected format
    std::cout << render_time << std::endl;
    std::cout << pixels_per_sec << std::endl;
    std::cout << memory_usage << std::endl;
    
    return 0;
}