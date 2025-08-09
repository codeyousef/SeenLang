use std::time::Instant;
use std::env;

// Simple ray tracer benchmark implementation
#[derive(Clone, Copy)]
struct Vec3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Vec3 {
    fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }
    
    fn add(&self, other: &Vec3) -> Vec3 {
        Vec3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
    
    fn sub(&self, other: &Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
    
    fn mul(&self, t: f64) -> Vec3 {
        Vec3::new(self.x * t, self.y * t, self.z * t)
    }
    
    fn dot(&self, other: &Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }
    
    fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
    
    fn normalize(&self) -> Vec3 {
        let l = self.length();
        Vec3::new(self.x / l, self.y / l, self.z / l)
    }
}

struct Ray {
    origin: Vec3,
    direction: Vec3,
}

impl Ray {
    fn new(origin: Vec3, direction: Vec3) -> Self {
        Ray { origin, direction: direction.normalize() }
    }
}

struct Sphere {
    center: Vec3,
    radius: f64,
    color: Vec3,
}

impl Sphere {
    fn new(center: Vec3, radius: f64, color: Vec3) -> Self {
        Sphere { center, radius, color }
    }
    
    fn intersect(&self, ray: &Ray) -> Option<f64> {
        let oc = ray.origin.sub(&self.center);
        let b = oc.dot(&ray.direction);
        let c = oc.dot(&oc) - self.radius * self.radius;
        let discriminant = b * b - c;
        
        if discriminant < 0.0 {
            return None;
        }
        
        let sqrt_d = discriminant.sqrt();
        let t1 = -b - sqrt_d;
        let t2 = -b + sqrt_d;
        
        if t1 > 0.0 {
            Some(t1)
        } else if t2 > 0.0 {
            Some(t2)
        } else {
            None
        }
    }
}

struct SimpleRayTracer {
    spheres: Vec<Sphere>,
    width: usize,
    height: usize,
}

impl SimpleRayTracer {
    fn new(width: usize, height: usize) -> Self {
        let mut spheres = Vec::new();
        spheres.push(Sphere::new(Vec3::new(0.0, 0.0, -5.0), 1.0, Vec3::new(1.0, 0.0, 0.0)));
        spheres.push(Sphere::new(Vec3::new(-2.0, 0.0, -6.0), 1.0, Vec3::new(0.0, 1.0, 0.0)));
        spheres.push(Sphere::new(Vec3::new(2.0, 0.0, -4.0), 0.5, Vec3::new(0.0, 0.0, 1.0)));
        spheres.push(Sphere::new(Vec3::new(0.0, -101.0, -5.0), 100.0, Vec3::new(0.5, 0.5, 0.5)));
        
        SimpleRayTracer { spheres, width, height }
    }
    
    fn trace(&self, ray: &Ray) -> Vec3 {
        let mut closest_t = f64::INFINITY;
        let mut hit_sphere = None;
        
        for sphere in &self.spheres {
            if let Some(t) = sphere.intersect(ray) {
                if t < closest_t {
                    closest_t = t;
                    hit_sphere = Some(sphere);
                }
            }
        }
        
        if let Some(sphere) = hit_sphere {
            sphere.color
        } else {
            // Sky gradient
            let t = 0.5 * (ray.direction.y + 1.0);
            Vec3::new(1.0, 1.0, 1.0).mul(1.0 - t).add(&Vec3::new(0.5, 0.7, 1.0).mul(t))
        }
    }
    
    fn render(&self) -> Vec<Vec3> {
        let mut pixels = Vec::with_capacity(self.width * self.height);
        
        for y in 0..self.height {
            for x in 0..self.width {
                let u = (x as f64 + 0.5) / self.width as f64 - 0.5;
                let v = (y as f64 + 0.5) / self.height as f64 - 0.5;
                
                let ray = Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(u, -v, -1.0));
                pixels.push(self.trace(&ray));
            }
        }
        
        pixels
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let width = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(200);
    let height = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(150);
    let samples = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    
    let tracer = SimpleRayTracer::new(width, height);
    
    let start = Instant::now();
    
    for _ in 0..samples {
        let _ = tracer.render();
    }
    
    let render_time = start.elapsed().as_secs_f64();
    let pixels_per_sec = (width * height * samples) as f64 / render_time;
    let memory_usage = std::mem::size_of::<Sphere>() * 4 + width * height * std::mem::size_of::<Vec3>();
    
    // Output in expected format
    println!("{}", render_time);
    println!("{}", pixels_per_sec);
    println!("{}", memory_usage);
}