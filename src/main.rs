use std::vec;

use eframe::egui::*;
// use egui::debug_text::print;
use glam::Vec3;

use rfd::FileDialog;
use std::path::PathBuf;
use tobj::LoadOptions;

fn main() -> eframe::Result {
    let mut three_d_engine = ThreeDEngine::new();

    three_d_engine.cube();

    return start_app(three_d_engine);
}

fn start_app<T: eframe::App + 'static>(app: T) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_inner_size([800.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native("3D Engine", options, Box::new(|_cc| Ok(Box::new(app))))
}

struct Bindings {
    forward: egui::Key,
    left: egui::Key,
    backward: egui::Key,
    right: egui::Key,
}

impl Bindings {
    fn qwerty() -> Self {
        Self {
            forward: egui::Key::W,
            left: egui::Key::A,
            backward: egui::Key::S,
            right: egui::Key::D,
        }
    }

    fn azerty() -> Self {
        Self {
            forward: egui::Key::Z,
            left: egui::Key::Q,
            backward: egui::Key::S,
            right: egui::Key::D,
        }
    }
}

struct ThreeDEngine {
    // RENDERING
    // TODO : Store Radians instead of Degrees for performance
    smoothed_fps: f32,
    camera_speed: f32,
    camera_position: Vec3,
    camera_rotation: Vec3, // Degrees : Yaw, Pitch, Roll
    fov: f32,              // Field of View (Degrees)
    stroke: Stroke,
    perspective: bool,
    display_vertices: bool,
    // LOGIC : Transformations
    bindings: Bindings,
    azerty: bool,
    // TODO : Objects List -> Manage Multiple Objects + Draw Origin
    model_position: Vec3,
    model_rotation: Vec3, // Degrees
    model_scale: Vec3,
    translate: bool,
    rotate: bool,
    scale: bool,
    translate_osciallator: f32,
    scale_osciallator: f32,
    // MODEL DATA
    // TODO : Separate Data / Engine
    vertices: Vec<glam::Vec3>,
    faces: Vec<Vec<u16>>, // TODO : Triangulate + Flatten
}

impl ThreeDEngine {
    fn new() -> Self {
        Self {
            // RENDERING
            smoothed_fps: 60.0,
            camera_speed: 0.05,
            camera_position: glam::Vec3::new(0.0, 0.0, 0.0),
            camera_rotation: glam::Vec3::new(0.0, 0.0, 0.0),
            fov: 90.0,
            stroke: egui::Stroke::new(2.0, egui::Color32::from_rgb(190, 110, 40)),
            perspective: true,
            display_vertices: true,
            // LOGIC : Transformations
            bindings: Bindings::qwerty(),
            azerty: false,
            model_position: glam::Vec3::new(0.0, 0.0, 1.0),
            model_rotation: Vec3::new(0.0, 0.0, 0.0),
            model_scale: Vec3::new(1.0, 1.0, 1.0),
            translate: false,
            rotate: true,
            scale: false,
            translate_osciallator: 0.0,
            scale_osciallator: 0.0,
            // MODEL DATA
            vertices: Vec::new(),
            faces: Vec::new(),
        }
    }

    // LOGIC : Transformations
    fn automatic_transform(&mut self, dt: f32) {
        // Model Translation
        if self.translate {
            self.translate_osciallator += dt;
            let amplitude = 0.01;
            let oscillation = self.translate_osciallator.sin() * amplitude;
            self.model_position.x += oscillation; // Oscillate horizontally
        }

        // Model Rotation
        if self.rotate {
            let angle = std::f32::consts::PI * dt; // 180 degrees per second
            self.model_rotation.y = (self.model_rotation.y + angle.to_degrees()) % 360.0;
        }

        // Model Scaling
        if self.scale {
            self.scale_osciallator += dt;
            let amplitude = 0.01;
            let oscillation = self.scale_osciallator.sin() * amplitude;
            self.model_scale += Vec3::new(oscillation, oscillation, oscillation);
        }
    }

    // RENDERING

    // Wireframe Rendering -> New Engine : Frame Image
    fn render_frame(&self, rect: &egui::Rect, painter: &egui::Painter) {
        let projection_function = if self.perspective {
            Self::perspective_project
        } else {
            Self::orthographic_project
        };

        let screen_points: Vec<Option<egui::Vec2>> = self.frame_image(&rect, &projection_function);

        // Render Vertices
        if self.display_vertices {
            for point in &screen_points {
                self.render_vertex(&rect, &painter, *point);
            }
        }

        // Render Edges
        for face in &self.faces {
            for i in 0..face.len() {
                self.render_edge(
                    &rect,
                    &painter,
                    screen_points[face[i] as usize],
                    screen_points[face[(i + 1) % face.len()] as usize],
                );
            }
        }
    }

    // Model -> Model Image (Model + Transformations) / World -> 2D Frustum (Projection) -> Screen Space
    fn frame_image(
        &self,
        rect: &egui::Rect,
        projection_function: &dyn Fn(&Self, &Vec3) -> Vec2,
    ) -> Vec<Option<egui::Vec2>> {
        let rotation_matrix_x = glam::Mat3::from_rotation_x(self.model_rotation.x.to_radians());
        let rotation_matrix_y = glam::Mat3::from_rotation_y(self.model_rotation.y.to_radians());
        let rotation_matrix_z = glam::Mat3::from_rotation_z(self.model_rotation.z.to_radians());
        let scale_matrix = glam::Mat3::from_diagonal(self.model_scale);

        return self
            .vertices
            .iter()
            .map(|v| {
                // 1] Model Rotation/Scale = Model -> World Space
                let mut world_v =
                    scale_matrix * rotation_matrix_z * rotation_matrix_y * rotation_matrix_x * *v;

                // 2] World Space -> View Space
                world_v += self.model_position;
                // Camera Position
                world_v = self.relative_vertex(&world_v);

                let cam_quat = glam::Quat::from_euler(
                    glam::EulerRot::YXZ,
                    self.camera_rotation.y.to_radians(),
                    self.camera_rotation.x.to_radians(),
                    self.camera_rotation.z.to_radians(),
                );

                // View Rotation = Camera Rotation inverse (conjugate)
                let view_quat = cam_quat.inverse(); // cam_quat.conjugate();
                let view_matrix = glam::Mat3::from_quat(view_quat);

                world_v = view_matrix * world_v;

                // 3] Projection
                return (world_v.z - self.camera_position.z > 0.1).then(|| {
                    Self::proj_to_screen(
                        &projection_function(&self, &world_v),
                        rect.width(),
                        rect.height(),
                    )
                });
            })
            .collect();
    }

    // World -> 2D Frustum (Perspective)
    fn perspective_project(&self, vertex: &Vec3) -> Vec2 {
        // let aspect_ratio = 1.0;
        let fov_rad = self.fov.to_radians();
        let f = 1.0 / (fov_rad * 0.5).tan();

        return Vec2::new(
            vertex.x * f / vertex.z, // / aspect_ratio
            vertex.y * f / vertex.z,
        );
    }

    // World -> 2D Frustum (Orthographic)
    fn orthographic_project(&self, vertex: &Vec3) -> Vec2 {
        let fov_rad = self.fov.to_radians();
        let f = 1.0 / (fov_rad * 0.5).tan();

        return Vec2::new(vertex.x * f, vertex.y * f);
    }

    fn relative_vertex(&self, vertex: &Vec3) -> Vec3 {
        return Vec3::new(
            vertex.x - self.camera_position.x,
            vertex.y - self.camera_position.y,
            vertex.z - self.camera_position.z,
        );
    }

    // 2D Frustum -> Screen space
    fn proj_to_screen(point: &Vec2, width: f32, height: f32) -> Vec2 {
        // Aspect Ratio Correction -> Resize Window
        let min = width.min(height);
        let x_offset = (width.max(height) - min) * 0.5;

        // -1..1 -> 0..2 -> 0..1 -> 0..width/height
        let x = (point.x + 1.0) / 2.0 * min + x_offset;
        let y = (1.0 - (point.y + 1.0) / 2.0) * min;
        return Vec2::new(x, y);
    }

    fn render_vertex(&self, rect: &egui::Rect, painter: &egui::Painter, point: Option<Vec2>) {
        if let Some(point) = point {
            let vertex_rect = Rect::from_center_size(rect.left_top() + point, vec2(10.0, 10.0));
            painter.rect_filled(vertex_rect, 0.0, self.stroke.color);
        }
    }

    fn render_edge(
        &self,
        rect: &egui::Rect,
        painter: &egui::Painter,
        p1: Option<Vec2>,
        p2: Option<Vec2>,
    ) {
        if let (Some(p1), Some(p2)) = (p1, p2) {
            painter.line_segment([rect.left_top() + p1, rect.left_top() + p2], self.stroke);
        }
    }

    // UTILS

    fn cube(&mut self) {
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
        self.vertices = vertices;
        self.faces = faces;
    }

    fn display_fps(&mut self, rect: &egui::Rect, painter: &egui::Painter, fps: f32) {
        let alpha = 0.05;
        self.smoothed_fps = (self.smoothed_fps * (1.0 - alpha)) + (fps * alpha);

        painter.text(
            rect.left_top() + egui::vec2(10.0, 10.0), // 10px padding from top-left
            egui::Align2::LEFT_TOP,
            format!("FPS: {:.2}", self.smoothed_fps),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
    }

    // Load OBJ
    fn pick_obj_file() -> Option<PathBuf> {
        let file = FileDialog::new()
            .add_filter("Object Files", &["obj"]) // Filter for .obj files
            .set_directory("/") // Starting directory
            .pick_file();

        return file;
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

    // OLD ENGINE

    fn old_engine(
        &mut self,
        dt: f32,
        rect: &egui::Rect,
        painter: &egui::Painter,
        projection_function: &dyn Fn(&Self, &Vec3) -> Vec2,
    ) {
        let angle = std::f32::consts::PI * dt; // 180 degrees per second
        let sin_angle = angle.sin();
        let cos_angle = angle.cos();

        // Render Vertices
        for vertex in &mut self.vertices {
            if self.rotate {
                // Maybe : StateMachine for automatic transformations
                // Self::rotate_y(vertex, angle); // Rotate
                Self::rotate_y_computed(vertex, sin_angle, cos_angle); // Rotate
            }

            if self.display_vertices {
                let vertex_world_pos = self.model_position + *vertex;

                if vertex_world_pos.z <= 0.0 {
                    continue; // Skip vertices behind the camera
                }

                let vertex_pos = Self::project_simple(&vertex_world_pos);
                let vertex_rect = Rect::from_center_size(
                    rect.left_top()
                        + Self::proj_to_screen(&vertex_pos, rect.width(), rect.height()),
                    vec2(10.0, 10.0),
                );
                painter.rect_filled(vertex_rect, 0.0, self.stroke.color);
            }
        }

        for face in &self.faces {
            for i in 0..face.len() {
                let v1_world_pos = self.model_position + self.vertices[face[i] as usize];
                let v2_world_pos =
                    self.model_position + self.vertices[face[(i + 1) % face.len()] as usize];

                if v1_world_pos.z <= 0.0 || v2_world_pos.z <= 0.0 {
                    continue; // Skip vertices behind the camera
                }

                let p1 = Self::proj_to_screen(
                    &projection_function(&self, &v1_world_pos),
                    rect.width(),
                    rect.height(),
                );
                let p2 = Self::proj_to_screen(
                    &projection_function(&self, &v2_world_pos),
                    rect.width(),
                    rect.height(),
                );

                painter.line_segment([rect.left_top() + p1, rect.left_top() + p2], self.stroke);
            }
        }
    }

    fn project_simple(vertex: &Vec3) -> Vec2 {
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
}

impl eframe::App for ThreeDEngine {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // ui.request_repaint();
            ui.request_repaint_after(std::time::Duration::from_millis(16)); // 60 FPS
            let dt = ui.input(|i| i.stable_dt); // DeltaTime in second
            let fps = 1.0 / dt;

            // INTERFACE

            // Settings : Import OBJ, Reset, Perspective, Render Vertices, Bindings
            ui.horizontal(|ui| {
                if ui.button("Import OBJ").clicked() {
                    let file = Self::pick_obj_file();

                    if let Some(path) = file {
                        self.load_obj_custom(path.to_str().unwrap());
                    }
                }

                if ui.button("Reset").clicked() {
                    *self = Self::new();
                    self.cube();
                }

                ui.checkbox(&mut self.perspective, "Perspective");
                ui.add(
                    egui::DragValue::new(&mut self.fov)
                        .prefix("FOV: ")
                        .speed(0.1)
                        .range(10.0..=170.0),
                );
                ui.checkbox(&mut self.display_vertices, "Render Vertices");

                if ui.checkbox(&mut self.azerty, "AZERTY").clicked() {
                    if self.azerty {
                        self.bindings = Bindings::azerty();
                    } else {
                        self.bindings = Bindings::qwerty();
                    }
                }
            });

            // Manual Transformations
            ui.horizontal(|ui| {
                ui.label("Model");

                // Model Position
                ui.label("Position :");
                ui.add(
                    egui::DragValue::new(&mut self.model_position.x)
                        .prefix("X: ")
                        .speed(0.01),
                );
                ui.add(
                    egui::DragValue::new(&mut self.model_position.y)
                        .prefix("Y: ")
                        .speed(0.01),
                );
                ui.add(
                    egui::DragValue::new(&mut self.model_position.z)
                        .prefix("Z: ")
                        .speed(0.01),
                );

                // Model Rotation
                ui.label("Rotation :");
                let response = ui.add(
                    egui::DragValue::new(&mut self.model_rotation.x)
                        .prefix("X: ")
                        .speed(0.05)
                        .range(-360.0..=360.0),
                );

                // if response.changed() {
                //     println!("Rotation X is now: {}", self.model_rotation.x);
                //     // Change to radians
                // }

                ui.add(
                    egui::DragValue::new(&mut self.model_rotation.y)
                        .prefix("Y: ")
                        .speed(0.05)
                        .range(-360.0..=360.0),
                );
                ui.add(
                    egui::DragValue::new(&mut self.model_rotation.z)
                        .prefix("Z: ")
                        .speed(0.05)
                        .range(-360.0..=360.0),
                );

                // Model Scale
                ui.label("Scale :");
                ui.add(
                    egui::DragValue::new(&mut self.model_scale.x)
                        .prefix("X: ")
                        .speed(0.01)
                        .range(0.0..=10.0),
                );
                ui.add(
                    egui::DragValue::new(&mut self.model_scale.y)
                        .prefix("Y: ")
                        .speed(0.01)
                        .range(0.0..=10.0),
                );
                ui.add(
                    egui::DragValue::new(&mut self.model_scale.z)
                        .prefix("Z: ")
                        .speed(0.01)
                        .range(0.0..=10.0),
                );
            });

            // Automatic Transformations
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.rotate, "Rotate");
                ui.checkbox(&mut self.translate, "Translate");
                ui.checkbox(&mut self.scale, "Scale");
            });

            // LOGIC

            // Camera Controls
            ui.input(|i| {
                if i.key_down(self.bindings.forward) {
                    self.camera_position.z += self.camera_speed; // Forward
                } else if i.key_down(self.bindings.backward) {
                    self.camera_position.z -= self.camera_speed; // Backward
                }

                if i.key_down(self.bindings.left) {
                    self.camera_position.x -= self.camera_speed; // Left
                } else if i.key_down(self.bindings.right) {
                    self.camera_position.x += self.camera_speed; // Right
                }

                if i.pointer.secondary_down() {
                    let delta = i.pointer.delta();
                    self.camera_rotation.y += delta.x * 0.1; // Yaw
                    self.camera_rotation.x += delta.y * 0.1; // Pitch
                }
            });

            self.automatic_transform(dt);

            // 3D RENDERING

            // Draw Area
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

            self.render_frame(&rect, &painter);

            self.display_fps(&rect, &painter, fps);
        });
    }
}
