"""RendererCanvas —— 软件渲染画布。每帧 _draw_self 自动渲染。

受 ``config.debug.ui.render_profile`` 控制时记录平均帧率，退出时打印。
"""

import atexit
import time

import numpy as np
import pygame

from musesp_ui.components.core import Component
from musesp_ui.profile import profile
from musesp_ui.renderer.pipeline import Pipeline
from musesp_ui.renderer.shader import Shader
from musesp_config.config import config

# ---- FPS 追踪（受 config.debug.ui.render_profile 控制） ----
_fps_registry: list["RendererCanvas"] = []


def _print_fps() -> None:
    if not _fps_registry:
        return
    print("\n--- RendererCanvas 平均帧率 ---")
    for i, rc in enumerate(_fps_registry):
        if rc._frame_count == 0:
            continue
        avg = rc._frame_count / rc._frame_total_time
        print(f"  [#{i}] {rc._frame_count} 帧 / {rc._frame_total_time:.2f}s"
              f" → 平均 {avg:.1f} FPS"
              f"  ({rc.width}×{rc.height})")
    print("-------------------------------")


if config.debug.ui.render_profile:
    atexit.register(_print_fps)


class RendererCanvas(Component):

    def __init__(self, shader: Shader, vbo: np.ndarray, ibo: np.ndarray,
                 x: int = 0, y: int = 0, width: int = 0, height: int = 0):
        super().__init__(x=x, y=y, width=width, height=height)
        self._shader = shader
        self._vbo = vbo
        self._ibo = ibo
        self._pipeline: Pipeline | None = None
        if config.debug.ui.render_profile:
            self._frame_count = 0
            self._frame_total_time = 0.0
            self._last_frame_time = 0.0
            _fps_registry.append(self)

    @profile
    def _ensure_pipeline(self):
        if self._pipeline is None or (
            self._pipeline.width != self.width or
            self._pipeline.height != self.height
        ):
            self._pipeline = Pipeline(self.width, self.height)

    @profile
    def _draw_self(self, surface, draw_x, draw_y):
        if config.debug.ui.render_profile:
            self._last_frame_time = time.perf_counter()
        self._ensure_pipeline()
        self._pipeline.clear((0, 0, 0, 255))
        self._pipeline.draw_triangles(self._vbo, self._ibo, self._shader)
        self._pipeline.blit_to(surface, draw_x, draw_y)
        if config.debug.ui.render_profile:
            elapsed = time.perf_counter() - self._last_frame_time
            self._frame_total_time += elapsed
            self._frame_count += 1
