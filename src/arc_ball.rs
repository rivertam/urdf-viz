/*
Based on https://github.com/sebcrozet/kiss3d/blob/master/src/camera/arc_ball.rs
(Because I don't know how to fix set_up_axis, I modified to use Z-up always.)

Copyright (c) 2013, Sébastien Crozet
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the author nor the names of its contributors may be used
   to endorse or promote products derived from this software without specific
   prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/
use kiss3d::camera::Camera;
use kiss3d::event::{Action, Key, MouseButton, WindowEvent};
use kiss3d::resource::ShaderUniform;
use kiss3d::window::Canvas;
use na::{self, Isometry3, Matrix4, Perspective3, Point3, Vector2, Vector3};
use std::f32;

/// Arc-ball camera mode.
///
/// An arc-ball camera is a camera rotating around a fixed point (the focus point) and always
/// looking at it. The following inputs are handled:
///
/// * Left button press + drag - rotates the camera around the focus point
/// * Right button press + drag - translates the focus point on the plane orthogonal to the view
/// direction
/// * Scroll in/out - zoom in/out
/// * Enter key - set the focus point to the origin
#[derive(Clone, Debug)]
pub struct ArcBall {
    /// The focus point.
    at: Point3<f32>,
    /// Yaw of the camera (rotation along the y axis).
    yaw: f32,
    /// Pitch of the camera (rotation along the x axis).
    pitch: f32,
    /// Distance from the camera to the `at` focus point.
    dist: f32,
    /// Minimum distance from the camera to the `at` focus point.
    min_dist: f32,
    /// Maximum distance from the camera to the `at` focus point.
    max_dist: f32,

    /// Increment of the yaw per unit mouse movement. The default value is 0.005.
    yaw_step: f32,
    /// Increment of the pitch per unit mouse movement. The default value is 0.005.
    pitch_step: f32,
    /// Minimum pitch of the camera.
    min_pitch: f32,
    /// Maximum pitch ofthe camera.
    max_pitch: f32,
    /// Increment of the distance per unit scrolling. The default value is 40.0.
    dist_step: f32,
    rotate_button: Option<MouseButton>,
    drag_button: Option<MouseButton>,
    reset_key: Option<Key>,

    projection: Perspective3<f32>,
    view: Matrix4<f32>,
    proj: Matrix4<f32>,
    proj_view: Matrix4<f32>,
    inverse_proj_view: Matrix4<f32>,
    last_cursor_pos: Vector2<f32>,
    up_axis: Vector3<f32>,
}

impl ArcBall {
    /// Create a new arc-ball camera.
    pub fn new(eye: Point3<f32>, at: Point3<f32>) -> ArcBall {
        ArcBall::new_with_frustrum(f32::consts::PI / 4.0, 0.1, 1024.0, eye, at)
    }

    /// Creates a new arc ball camera with default sensitivity values.
    pub fn new_with_frustrum(
        fov: f32,
        znear: f32,
        zfar: f32,
        eye: Point3<f32>,
        at: Point3<f32>,
    ) -> ArcBall {
        let mut res = ArcBall {
            at: Point3::new(0.0, 0.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            dist: 0.0,
            min_dist: 0.00001,
            max_dist: std::f32::MAX,
            yaw_step: 0.005,
            pitch_step: 0.005,
            min_pitch: 0.01,
            max_pitch: std::f32::consts::PI - 0.01,
            dist_step: 40.0,
            rotate_button: Some(MouseButton::Button1),
            drag_button: Some(MouseButton::Button2),
            reset_key: Some(Key::Return),
            projection: Perspective3::new(800.0 / 600.0, fov, znear, zfar),
            view: na::zero(),
            proj: na::zero(),
            proj_view: na::zero(),
            inverse_proj_view: na::zero(),
            last_cursor_pos: na::zero(),
            up_axis: Vector3::y(),
        };

        res.look_at(eye, at);

        res
    }

    /// The point the arc-ball is looking at.
    pub fn at(&self) -> Point3<f32> {
        self.at
    }

    /// Get a mutable reference to the point the camera is looking at.
    pub fn set_at(&mut self, at: Point3<f32>) {
        self.at = at;
        self.update_projviews();
    }

    /// The arc-ball camera `yaw`.
    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    /// Sets the camera `yaw`. Change this to modify the rotation along the `up` axis.
    pub fn set_yaw(&mut self, yaw: f32) {
        self.yaw = yaw;

        self.update_restrictions();
        self.update_projviews();
    }

    /// The arc-ball camera `pitch`.
    pub fn pitch(&self) -> f32 {
        self.pitch
    }

    /// Sets the camera `pitch`.
    pub fn set_pitch(&mut self, pitch: f32) {
        self.pitch = pitch;

        self.update_restrictions();
        self.update_projviews();
    }

    /// The minimum pitch of the camera.
    pub fn min_pitch(&self) -> f32 {
        self.min_pitch
    }

    /// Set the minimum pitch of the camera.
    pub fn set_min_pitch(&mut self, min_pitch: f32) {
        self.min_pitch = min_pitch;
    }

    /// The maximum pitch of the camera.
    pub fn max_pitch(&self) -> f32 {
        self.max_pitch
    }

    /// Set the maximum pitch of the camera.
    pub fn set_max_pitch(&mut self, max_pitch: f32) {
        self.max_pitch = max_pitch;
    }

    /// The distance from the camera position to its view point.
    pub fn dist(&self) -> f32 {
        self.dist
    }

    /// Move the camera such that it is at a given distance from the view point.
    pub fn set_dist(&mut self, dist: f32) {
        self.dist = dist;

        self.update_restrictions();
        self.update_projviews();
    }

    /// The minimum distance from the camera position to its view point.
    pub fn min_dist(&self) -> f32 {
        self.min_dist
    }

    /// Set the minimum distance from the camera position to its view point.
    pub fn set_min_dist(&mut self, min_dist: f32) {
        self.min_dist = min_dist;
    }

    /// The maximum distance from the camera position to its view point.
    pub fn max_dist(&self) -> f32 {
        self.max_dist
    }

    /// Set the maximum distance from the camera position to its view point.
    pub fn set_max_dist(&mut self, max_dist: f32) {
        self.max_dist = max_dist;
    }

    /// Move and orient the camera such that it looks at a specific point.
    pub fn look_at(&mut self, eye: Point3<f32>, at: Point3<f32>) {
        let dist = (eye - at).norm();
        let pitch = ((eye.y - at.y) / dist).acos();
        let yaw = (eye.z - at.z).atan2(eye.x - at.x);

        self.at = at;
        self.dist = dist;
        self.yaw = yaw;
        self.pitch = pitch;

        self.update_restrictions();
        self.update_projviews();
    }

    /// Transformation applied by the camera without perspective.
    fn update_restrictions(&mut self) {
        if self.dist < self.min_dist {
            self.dist = self.min_dist
        }

        if self.dist > self.max_dist {
            self.dist = self.max_dist
        }

        if self.pitch <= self.min_pitch {
            self.pitch = self.min_pitch
        }

        if self.pitch > self.max_pitch {
            self.pitch = self.max_pitch
        }
    }

    /// The button used to rotate the ArcBall camera.
    pub fn rotate_button(&self) -> Option<MouseButton> {
        self.rotate_button
    }

    /// Set the button used to rotate the ArcBall camera.
    /// Use None to disable rotation.
    pub fn rebind_rotate_button(&mut self, new_button: Option<MouseButton>) {
        self.rotate_button = new_button;
    }

    /// The button used to drag the ArcBall camera.
    pub fn drag_button(&self) -> Option<MouseButton> {
        self.drag_button
    }

    /// Set the button used to drag the ArcBall camera.
    /// Use None to disable dragging.
    pub fn rebind_drag_button(&mut self, new_button: Option<MouseButton>) {
        self.drag_button = new_button;
    }

    /// The key used to reset the ArcBall camera.
    pub fn reset_key(&self) -> Option<Key> {
        self.reset_key
    }

    /// Set the key used to reset the ArcBall camera.
    /// Use None to disable reset.
    pub fn rebind_reset_key(&mut self, new_key: Option<Key>) {
        self.reset_key = new_key;
    }

    fn handle_left_button_displacement(&mut self, dpos: &Vector2<f32>) {
        self.yaw = self.yaw - dpos.x * self.yaw_step;
        self.pitch = self.pitch - dpos.y * self.pitch_step;

        self.update_restrictions();
        self.update_projviews();
    }

    fn handle_right_button_displacement(&mut self, dpos: &Vector2<f32>) {
        let eye = self.eye();
        let dir = (self.at - eye).normalize();
        let tangent = self.up_axis.cross(&dir).normalize();
        let bitangent = dir.cross(&tangent);
        let mult = self.dist / 1000.0;

        self.at = self.at + tangent * (dpos.x * mult) + bitangent * (dpos.y * mult);
        self.update_projviews();
    }

    fn handle_scroll(&mut self, off: f32) {
        self.dist = self.dist + self.dist_step * (off) / 120.0;
        self.update_restrictions();
        self.update_projviews();
    }

    fn update_projviews(&mut self) {
        self.proj = *self.projection.as_matrix();
        self.view = self.view_transform().to_homogeneous();
        self.proj_view = self.proj * self.view;
        self.inverse_proj_view = self.proj_view.try_inverse().unwrap();
    }

    /// Sets the up vector of this camera.
    #[inline]
    pub fn set_up_axis(&mut self, up_axis: Vector3<f32>) {
        self.up_axis = up_axis;
    }
}

impl Camera for ArcBall {
    fn clip_planes(&self) -> (f32, f32) {
        (self.projection.znear(), self.projection.zfar())
    }

    fn view_transform(&self) -> Isometry3<f32> {
        Isometry3::look_at_rh(&self.eye(), &self.at, &self.up_axis)
    }

    fn eye(&self) -> Point3<f32> {
        let px = self.at.x + self.dist * self.yaw.cos() * self.pitch.sin();
        let py = self.at.y + self.dist * self.yaw.sin() * self.pitch.sin();
        let pz = self.at.z + self.dist * self.pitch.cos();

        Point3::new(px, py, pz)
    }

    fn handle_event(&mut self, canvas: &Canvas, event: &WindowEvent) {
        match *event {
            WindowEvent::CursorPos(x, y, _) => {
                let curr_pos = Vector2::new(x as f32, y as f32);

                if let Some(rotate_button) = self.rotate_button {
                    if canvas.get_mouse_button(rotate_button) == Action::Press {
                        let dpos = curr_pos - self.last_cursor_pos;
                        self.handle_left_button_displacement(&dpos)
                    }
                }

                if let Some(drag_button) = self.drag_button {
                    if canvas.get_mouse_button(drag_button) == Action::Press {
                        let dpos = curr_pos - self.last_cursor_pos;
                        self.handle_right_button_displacement(&dpos)
                    }
                }

                self.last_cursor_pos = curr_pos;
            }
            WindowEvent::Key(key, Action::Press, _) if Some(key) == self.reset_key => {
                self.at = Point3::origin();
                self.update_projviews();
            }
            WindowEvent::Scroll(_, off, _) => self.handle_scroll(off as f32),
            WindowEvent::FramebufferSize(w, h) => {
                self.projection.set_aspect(w as f32 / h as f32);
                self.update_projviews();
            }
            _ => {}
        }
    }

    #[inline]
    fn upload(
        &self,
        _: usize,
        proj: &mut ShaderUniform<Matrix4<f32>>,
        view: &mut ShaderUniform<Matrix4<f32>>,
    ) {
        proj.upload(&self.proj);
        view.upload(&self.view);
    }

    fn transformation(&self) -> Matrix4<f32> {
        self.proj_view
    }

    fn inverse_transformation(&self) -> Matrix4<f32> {
        self.inverse_proj_view
    }

    fn update(&mut self, _: &Canvas) {}
}
