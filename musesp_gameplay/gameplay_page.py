"""玩法页面 —— 铺满全屏的软渲染画布。"""

import numpy as np
import pygame

from musesp_ui.application import RunMode
from musesp_ui.components.core import Constraintable
from musesp_ui.components.image_button import ImageButton
from musesp_ui.components.renderer_canvas import RendererCanvas
from musesp_ui.router import Page
from musesp_gameplay.game_shader import GameShader
from musesp_gameplay.menu_page import MenuPage


class GameplayPage(Page):
    def full_shadow_promise(self) -> bool:
        return True

    def on_activate(self) -> None:
        self._router._app.set_mode(RunMode.VSYNC)

    def build(self) -> None:
        self._router._app.set_mode(RunMode.VSYNC)

        shader = GameShader(camera_distance=5.0, fov=60.0)
        vbo, ibo = self._build_test_geometry()
        canvas = RendererCanvas(shader, vbo, ibo)
        canvas.h_constraint = Constraintable.MAXIMUM
        canvas.v_constraint = Constraintable.MAXIMUM
        self.add_component(canvas)

        menu_btn = ImageButton("assets/ui/menu_button.svg",
                               x=16, y=16, width=44, height=44)
        menu_btn.h_constraint = Constraintable.NONE
        menu_btn.v_constraint = Constraintable.NONE
        menu_btn.bind_on_mouse_click(self._on_menu)
        self._menu_btn = menu_btn
        self._root.add_sub_component(menu_btn)

    def _on_menu(self, event) -> bool:
        self._router.push(MenuPage())
        return False

    @staticmethod
    def _build_test_geometry():
        """构建测试几何：半透明白色正方体（边长 2，中心原点）。"""
        s = 1.0  # 半边长
        cf = 0.0
        # 8 个顶点
        verts = [
            [-s, -s, -s, cf], [ s, -s, -s, cf], [ s,  s, -s, cf], [-s,  s, -s, cf],
            [-s, -s,  s, cf], [ s, -s,  s, cf], [ s,  s,  s, cf], [-s,  s,  s, cf],
        ]
        # 12 个三角形（6 个面，每面 2 个三角形）
        indices = [
            0,1,2, 0,2,3,  # 前
            4,5,6, 4,6,7,  # 后
            0,1,5, 0,5,4,  # 下
            2,3,7, 2,7,6,  # 上
            0,3,7, 0,7,4,  # 左
            1,2,6, 1,6,5,  # 右
        ]
        vbo = np.array(verts, dtype=np.float32)
        ibo = np.array(indices, dtype=np.int32)
        return vbo, ibo

    def dispatch_event(self, event) -> bool:
        if event.type == pygame.KEYDOWN and event.key == pygame.K_ESCAPE:
            dummy = pygame.event.Event(pygame.MOUSEBUTTONUP, {"pos": (0, 0)})
            self._menu_btn._emit("mouse_click", dummy)
            return False
        return super().dispatch_event(event)
