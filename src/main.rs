use std::vec;

use eframe::egui::*;
use glam::Vec3;

use rfd::FileDialog;
use std::path::PathBuf;
use tobj::LoadOptions;

fn main() -> eframe::Result {
    let mut three_d_engine = ThreeDEngine::new();

    // Data
    // Cube
    let vertices = vec![
        // Front Face
        Vec3::new(0.25, 0.25, 0.25),
        Vec3::new(-0.25, 0.25, 0.25),
        Vec3::new(-0.25, -0.25, 0.25),
        Vec3::new(0.25, -0.25, 0.25),
        // Back Face
        Vec3::new(0.25, 0.25, -0.25),
        Vec3::new(-0.25, 0.25, -0.25),
        Vec3::new(-0.25, -0.25, -0.25),
        Vec3::new(0.25, -0.25, -0.25),
    ];

    // TODO : Triangulate + Flatten
    let faces: Vec<Vec<u16>> = vec![
        vec![0, 1, 2, 3], // Front
        vec![4, 5, 6, 7], // Back
        vec![0, 4],
        vec![1, 5],
        vec![2, 6],
        vec![3, 7],
        // Full Faces
        // vec![0, 4, 7, 3], // Right
        // vec![1, 5, 6, 2], // Left
        // vec![0, 1, 5, 4], // Top
        // vec![3, 2, 6, 7], // Bottom
    ];

    // Engine Setup
    three_d_engine.position.z = 1.0;
    three_d_engine.vertices = vertices;
    three_d_engine.faces = faces;

    return start_app(three_d_engine);
}

fn start_app<T: eframe::App + 'static>(app: T) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([800.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native("3D Engine", options, Box::new(|_cc| Ok(Box::new(app))))
}

struct ThreeDEngine {
    // TODO : Objects List -> Manage Multiple Objects
    position: Vec3,
    vertices: Vec<glam::Vec3>,
    faces: Vec<Vec<u16>>,
    stroke_color: Color32,
    stroke_width: f32,
    display_vertices: bool,
    rotate: bool,
}

impl ThreeDEngine {
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
            faces: Vec::new(),
            stroke_color: egui::Color32::from_rgb(190, 110, 40),
            stroke_width: 2.0,
            position: glam::Vec3::new(0.0, 0.0, 0.0),
            display_vertices: true,
            rotate: true,
        }
    }

    fn proj_to_screen(point: &Vec2, width: f32, height: f32) -> Vec2 {
        // -1..1 -> 0..2 -> 0..1 -> 0..width/height
        let min = width.min(height);
        let x_offset = width.max(height) - min;
        let x = (point.x + 1.0) / 2.0 * min + x_offset / 2.0;
        let y = (1.0 - (point.y + 1.0) / 2.0) * min;
        return Vec2::new(x, y);
    }

    fn project(vertex: &Vec3) -> Vec2 {
        return Vec2::new(vertex.x / vertex.z, vertex.y / vertex.z);
    }

    // Transformations
    // Rotations -> angle = radians
    fn rotate_y(vertex: &mut Vec3, angle: f32) {
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();
        let x = vertex.x * cos_angle - vertex.z * sin_angle;
        let z = vertex.x * sin_angle + vertex.z * cos_angle;
        vertex.x = x;
        vertex.z = z;
    }

    fn rotate_y_computed(vertex: &mut Vec3, sin_angle: f32, cos_angle: f32) {
        let x = vertex.x * cos_angle - vertex.z * sin_angle;
        let z = vertex.x * sin_angle + vertex.z * cos_angle;
        vertex.x = x;
        vertex.z = z;
    }

    // Scale

    fn pick_obj_file() -> Option<PathBuf> {
        let file = FileDialog::new()
            .add_filter("Object Files", &["obj"]) // Filter for .obj files
            .set_directory("/") // Starting directory
            .pick_file(); // This "blocks" until the user picks or cancels

        file // Returns Some(PathBuf) or None
    }

    fn load_obj_custom(&mut self, path: &str) {
        // 1. Load the file
        let (models, _) = tobj::load_obj(
            path,
            &LoadOptions {
                triangulate: true, // Converts quads to triangles automatically
                single_index: true,
                ..Default::default()
            },
        )
        .expect("Failed to load OBJ file");

        let mesh = &models[0].mesh;

        // 2. Convert flat f32 vec [x,y,z, x,y,z] to Vec<Vec3>
        let vertices: Vec<Vec3> = mesh
            .positions
            .chunks_exact(3)
            .map(|p| Vec3::new(p[0], p[1], p[2]))
            .collect();

        // 3. Convert flat indices [0,1,2, 3,4,5] to Vec<Vec<u8>>
        // Since we triangulated, each face has exactly 3 indices.
        let faces: Vec<Vec<u16>> = mesh
            .indices
            .chunks_exact(3)
            .map(|f| f.iter().map(|&i| i as u16).collect())
            .collect();

        self.vertices = vertices;
        self.faces = faces;
    }
}

impl eframe::App for ThreeDEngine {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.request_repaint_after(std::time::Duration::from_millis(16)); // 60 FPS
            let dt = ui.input(|i| i.stable_dt); // DeltaTime

            // Interactions

            // Import OBJ

            ui.horizontal(|ui| {
                if ui.button("Clear").clicked() {
                    self.vertices.clear();
                    self.faces.clear();
                }

                ui.checkbox(&mut self.display_vertices, "Display Vertices");
                ui.checkbox(&mut self.rotate, "Rotate");
            });

            // Draw area

            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::hover());
            let rect = response.rect;

            // Border
            painter.rect_stroke(
                rect,
                5.0,
                egui::Stroke::new(2.0, egui::Color32::GREEN),
                egui::StrokeKind::Middle,
            );

            let angle = std::f32::consts::PI * dt; // 180 degrees per second
            let sin_angle = angle.sin();
            let cos_angle = angle.cos();

            // Display Vertices
            for vertex in &mut self.vertices {
                if self.rotate { // TODO : StateMachine
                    // Self::rotate_y(vertex, angle); // Rotate
                    Self::rotate_y_computed(vertex, sin_angle, cos_angle); // Rotate
                }

                // self.position.z += 0.005; // Translate

                if self.display_vertices {
                    let vertex_pos = Self::project(&(self.position + *vertex)); // Project
                    let vertex_rect = Rect::from_center_size(
                        rect.left_top()
                            + Self::proj_to_screen(&vertex_pos, rect.width(), rect.height()),
                        vec2(10.0, 10.0),
                    );
                    painter.rect_filled(vertex_rect, 0.0, self.stroke_color);
                }
            }

            // Draw Edges
            for face in &self.faces {
                for i in 0..face.len() {
                    let v1 = self.vertices[face[i] as usize];
                    let v2 = self.vertices[face[(i + 1) % face.len()] as usize];

                    let p1 = Self::proj_to_screen(
                        &Self::project(&(self.position + v1)),
                        rect.width(),
                        rect.height(),
                    );
                    let p2 = Self::proj_to_screen(
                        &Self::project(&(self.position + v2)),
                        rect.width(),
                        rect.height(),
                    );

                    painter.line_segment(
                        [rect.left_top() + p1, rect.left_top() + p2],
                        egui::Stroke::new(self.stroke_width, self.stroke_color),
                    );
                }
            }
        });
    }
}
