import pygame

from musesp_ui.components.button import Button
from musesp_ui.components.label import Label
from musesp_ui.components.spacer import Spacer
from musesp_ui.components.core import Component, Constraintable, Direction
from musesp_ui.application import RunMode
from musesp_ui.router import Page
from musesp.pages.music_list import MusicListPage


class HomePage(Page):
    def full_shadow_promise(self) -> bool:
        return True

    def hide_last(self) -> None:
        pass

    def on_activate(self) -> None:
        self._router._app.set_mode(RunMode.EVENT)

    def build(self) -> None:
        self._router._app.set_mode(RunMode.EVENT)
        # 纵向内容容器
        content = Component()
        content.layout_direction = Direction.VERTICAL
        content.h_constraint = Constraintable.MAXIMUM
        content.v_constraint = Constraintable.MINIMUM

        title = Label("MuseSP", font_size=72, color=(255, 255, 255))
        title.v_constraint = Constraintable.MINIMUM
        title.h_constraint = Constraintable.MINIMUM
        title.min_height = 120
        title.min_width = 400
        content.add_sub_component(title)

        spacer = Spacer()
        spacer.v_constraint = Constraintable.MINIMUM
        spacer.h_constraint = Constraintable.MAXIMUM
        spacer.min_height = 30
        content.add_sub_component(spacer)

        btn_start = Button("开始")
        btn_start.v_constraint = Constraintable.MINIMUM
        btn_start.h_constraint = Constraintable.MAXIMUM
        btn_start.min_height = 50
        btn_start.min_width = 200
        btn_start.bind_on_mouse_click(
            lambda e: (print("[click] 开始"), False)[1])
        btn_start.bind_on_mouse_click(self._on_start)
        content.add_sub_component(btn_start)

        spacer_btn = Spacer()
        spacer_btn.v_constraint = Constraintable.MINIMUM
        spacer_btn.min_height = 10
        content.add_sub_component(spacer_btn)

        btn_settings = Button("设置")
        btn_settings.v_constraint = Constraintable.MINIMUM
        btn_settings.h_constraint = Constraintable.MAXIMUM
        btn_settings.min_height = 50
        btn_settings.min_width = 200
        content.add_sub_component(btn_settings)

        spacer_btn2 = Spacer()
        spacer_btn2.v_constraint = Constraintable.MINIMUM
        spacer_btn2.min_height = 10
        content.add_sub_component(spacer_btn2)

        btn_exit = Button("退出")
        btn_exit.v_constraint = Constraintable.MINIMUM
        btn_exit.h_constraint = Constraintable.MAXIMUM
        btn_exit.min_height = 50
        btn_exit.min_width = 200
        btn_exit.bind_on_mouse_click(
            lambda e: (pygame.event.post(pygame.event.Event(pygame.QUIT)), False)[1])
        content.add_sub_component(btn_exit)

        # 根组件改为横向排列
        self._root.layout_direction = Direction.HORIZONTAL

        self._spacer_left = Spacer()
        self._spacer_left.h_constraint = Constraintable.MAXIMUM
        self._spacer_left.v_constraint = Constraintable.MINIMUM
        self.add_component(self._spacer_left)

        self.add_component(content)

        self._spacer_right = Spacer()
        self._spacer_right.h_constraint = Constraintable.MAXIMUM
        self._spacer_right.v_constraint = Constraintable.MINIMUM
        self.add_component(self._spacer_right)

    def prepare_layout(self) -> None:
        cap = self._root.width * 2 // 7
        self._spacer_left.max_width = cap
        self._spacer_right.max_width = cap

    def _on_start(self, event) -> bool:
        if self._router is not None:
            self._router.push(MusicListPage())
        return False
