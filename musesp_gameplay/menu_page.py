"""MenuPage —— 居中半透明菜单。"""

import pygame

from musesp_ui.components.core import Component, Constraintable, Direction
from musesp_ui.components.image_button import ImageButton
from musesp_ui.components.spacer import Spacer
from musesp_ui.router import Page


class MenuPage(Page):
    """不占满窗口的菜单页面，居中矩形区域，半透明白色背景。"""

    def full_shadow_promise(self) -> bool:
        return False

    def build(self) -> None:
        self._root.layout_direction = Direction.VERTICAL

        # 上 Spacer
        top = Spacer()
        top.v_constraint = Constraintable.MAXIMUM
        self._root.add_sub_component(top)

        # 按钮行
        row = Component()
        row.layout_direction = Direction.HORIZONTAL
        row.v_constraint = Constraintable.MINIMUM
        row.h_constraint = Constraintable.MINIMUM
        row.min_height = 36

        btns: list[tuple[str, str, object]] = [
            ("assets/ui/cancel_button.svg", "取消", self._on_cancel),
            ("assets/ui/continue_button.svg", "继续", lambda e: False),
            ("assets/ui/replay_button.svg", "重来", lambda e: False),
            ("assets/ui/exit_button.svg", "退出", self._on_exit),
        ]
        for i, (path, label, handler) in enumerate(btns):
            if i == 0:
                s = Spacer()
                s.h_constraint = Constraintable.MINIMUM
                s.min_width = 8
                row.add_sub_component(s)

            btn = ImageButton(path, text=label, width=130, height=36,
                              font_size=14)
            btn.h_constraint = Constraintable.MINIMUM
            btn.v_constraint = Constraintable.MINIMUM
            btn.min_width = 120
            btn.min_height = 36
            btn.bind_on_mouse_click(handler)
            if label == "取消":
                self._cancel_btn = btn
            row.add_sub_component(btn)

            s = Spacer()
            s.h_constraint = Constraintable.MINIMUM
            s.min_width = 8
            row.add_sub_component(s)

        self._root.add_sub_component(row)

        # 下 Spacer
        bot = Spacer()
        bot.v_constraint = Constraintable.MAXIMUM
        self._root.add_sub_component(bot)

    def prepare_layout(self) -> None:
        """根据窗口尺寸计算菜单区域。"""
        rw, rh = self._root.width, self._root.height
        mw = rw * 2 // 5
        mh = rh // 4
        self._menu_x = (rw - mw) // 2
        self._menu_y = (rh - mh) // 2
        self._menu_w = mw
        self._menu_h = mh
        self._root.x = self._menu_x
        self._root.y = self._menu_y
        self._root.width = self._menu_w
        self._root.height = self._menu_h
        self._root.layout()

        bg = pygame.Surface((self._menu_w, self._menu_h), pygame.SRCALPHA)
        bg.fill((255, 255, 255, 180))
        self._bg_surf = bg

    def draw(self, surface: pygame.Surface) -> None:
        self.draw_background(surface, (self._menu_x, self._menu_y,
                                       self._menu_w, self._menu_h),
                             self._bg_surf)
        super().draw(surface)

    def _on_cancel(self, event) -> bool:
        self._router.pop()
        return False

    def _on_exit(self, event) -> bool:
        from musesp.pages.music_list import MusicListPage
        self._router.clear_and_push(MusicListPage())
        return False

    def dispatch_event(self, event) -> bool:
        if event.type == pygame.KEYDOWN and event.key == pygame.K_ESCAPE:
            dummy = pygame.event.Event(pygame.MOUSEBUTTONUP, {"pos": (0, 0)})
            self._cancel_btn._emit("mouse_click", dummy)
            return False
        if event.type in (pygame.MOUSEMOTION, pygame.MOUSEBUTTONDOWN,
                          pygame.MOUSEBUTTONUP):
            mx, my = event.pos
            if not (self._menu_x <= mx < self._menu_x + self._menu_w
                    and self._menu_y <= my < self._menu_y + self._menu_h):
                return True
        return self._root.dispatch_event(event)
