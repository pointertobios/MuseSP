"""软件渲染管线。"""

import numpy as np
import pygame

from musesp_ui.profile import profile
from musesp_ui.renderer.shader import Shader


class Pipeline:

    def __init__(self, width: int, height: int):
        self._fb = np.zeros((height, width, 4), dtype=np.uint8)
        self._w, self._h = width, height

    @property
    def width(self) -> int:
        return self._w

    @property
    def height(self) -> int:
        return self._h

    @profile
    def clear(self, color=(0, 0, 0, 255)):
        self._fb[:] = color

    @profile
    def draw_triangles(self, vbo: np.ndarray, ibo: np.ndarray, shader: Shader):
        out = shader.vertex(vbo)                 # (N, D_out)
        pos = out[:, :4]                          # (N, 4) NDC
        inv_w = np.float32(1.0 / np.maximum(np.abs(pos[:, 3]), 1e-8))
        sx = (pos[:, 0] * inv_w * 0.5 + 0.5) * self._w
        sy = (pos[:, 1] * inv_w * 0.5 + 0.5) * self._h
        sz = pos[:, 2] * inv_w

        # numpy 向量化提取所有三角形顶点
        ibo_2d = ibo.reshape(-1, 3)               # (T, 3)
        n_tris = len(ibo_2d)
        n_attr = out.shape[1] - 4                  # varyings 列数
        # screen_x, screen_y, screen_z, attrs...
        per_v = 3 + n_attr

        tri = np.empty((n_tris, 3, per_v), dtype=np.float64)
        for k, col in enumerate(ibo_2d.T):
            tri[:, k, 0] = sx[col]
            tri[:, k, 1] = sy[col]
            tri[:, k, 2] = sz[col]
            if n_attr > 0:
                tri[:, k, 3:] = out[col, 4:]

        # 逐三角形串行（并行在 rasterize 内部做块级并行）
        fb = self._fb
        for t in range(n_tris):
            shader.rasterize(tri[t, 0], tri[t, 1], tri[t, 2], fb, None)

    @profile
    def blit_to(self, surface: pygame.Surface, x: int, y: int) -> None:
        """直接将帧缓冲 RGB 写入目标 surface，避免中间 surface 拷贝。"""
        fb = self._fb  # (H, W, 4)
        try:
            target = pygame.surfarray.pixels3d(surface)  # (W_s, H_s, 3)
            target[x:x + self._w, y:y + self._h] = (
                fb[:, :, :3].transpose(1, 0, 2))
            del target  # 解锁 surface
        except (ValueError, pygame.error):
            # 不支持直接像素写入时回退
            surf = pygame.image.frombuffer(
                np.ascontiguousarray(fb), (self._w, self._h), "RGBA")
            surface.blit(surf, (x, y))
