"""游戏着色器。顶点：正交坐标 (x, y, z, color_flag)，自动绕 Y 轴旋转。"""

import time

import numpy as np

from musesp_ui.profile import profile
from musesp_ui.renderer.shader import Shader


class GameShader(Shader):

    def __init__(self, camera_distance: float = 5.0, fov: float = 60.0,
                 tilt: float = 25.0, rotation_speed: float = 0.5):
        """rotation_speed: 每秒旋转圈数，默认 0.5（2 秒一圈）。"""
        super().__init__()
        self._cam_dist = camera_distance
        fov_rad = np.radians(fov)
        self._f = 1.0 / np.tan(fov_rad / 2.0)
        tilt_rad = np.radians(tilt)
        self._cos_t = np.cos(tilt_rad)
        self._sin_t = np.sin(tilt_rad)
        self._rot_speed = rotation_speed
        self._start_time = time.perf_counter()

    @profile
    def vertex(self, vbo: np.ndarray) -> np.ndarray:
        """vbo: (N, 4) float32，列 [x, y, z, cf]（正交坐标）。

        先绕 Y 轴旋转，再经摄像机俯仰 → 透视投影 → NDC。
        """
        angle = (time.perf_counter() - self._start_time) * \
            self._rot_speed * 2.0 * np.pi
        cos_a = np.cos(angle)
        sin_a = np.sin(angle)

        x, y, z, cf = (vbo[:, 0], vbo[:, 1], vbo[:, 2], vbo[:, 3])

        # 绕 Y 轴旋转
        x_rot = x * cos_a + z * sin_a
        z_rot_world = -x * sin_a + z * cos_a
        y_rot_world = y

        # 摄像机绕 X 轴俯仰
        y_cam = y_rot_world * self._cos_t - z_rot_world * self._sin_t
        z_cam = y_rot_world * self._sin_t + z_rot_world * self._cos_t + self._cam_dist

        inv_z = 1.0 / np.maximum(np.abs(z_cam), 1e-6)
        N = vbo.shape[0]
        out = np.zeros((N, 8), dtype=np.float32)
        out[:, 0] = x_rot * self._f * inv_z
        out[:, 1] = y_cam * self._f * inv_z
        out[:, 2] = z_cam * inv_z
        out[:, 3] = 1.0
        out[:, 4] = cf
        return out

    @profile
    def fragment(self, varyings: np.ndarray) -> tuple[int, int, int, int]:
        return (255, 255, 255, 128)
