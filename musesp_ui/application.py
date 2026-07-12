"""Application —— 应用主循环。"""

import sys
from enum import Enum, auto

import pygame

from musesp_ui.router import Page, Router


class RunMode(Enum):
    """运行模式。"""
    EVENT = auto()  # 阻塞等待事件后刷新
    FPS = auto()    # 按目标帧率刷新
    VSYNC = auto()  # 垂直同步


class Application:
    def __init__(self, name: str, starts_with: Page | None = None):
        if starts_with is None:
            sys.exit()
        self.name = name
        pygame.init()
        self.router = Router(starts_with, app=self)
        self.screen = pygame.display.set_mode((1920, 1080), vsync=True)
        self.router._screen = self.screen
        self.router._win_w = self.screen.get_width()
        self.router._win_h = self.screen.get_height()
        self.router.current._root.width = self.router._win_w
        self.router.current._root.height = self.router._win_h
        self.router.current.prepare_layout()
        self.router.current._root.layout()
        pygame.display.set_caption(name)

        self._mode = RunMode.EVENT
        self._target_fps = 60
        self._clock = pygame.time.Clock()

    # ---- 模式切换（Page 可通过 self._router._app 访问） ----

    def set_mode(self, mode: RunMode, target_fps: int = 60) -> None:
        """切换运行模式。

        :param mode: 目标模式
        :param target_fps: FPS 模式下的目标帧率
        """
        self._mode = mode
        self._target_fps = target_fps

    @property
    def mode(self) -> RunMode:
        return self._mode

    # ---- 主循环 ----

    def run(self) -> None:
        running = True
        while running:
            if self._mode == RunMode.EVENT:
                running = self._run_event_loop(running)
            elif self._mode == RunMode.FPS:
                running = self._run_fps_loop(running)
            else:  # VSYNC — flip() 本身阻塞到垂直同步，无需 clock.tick
                running = self._run_vsync_loop(running)
        pygame.quit()

    def _process_events(self) -> bool:
        """处理事件队列，返回 False 表示收到 QUIT。"""
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                return False
            self.router.dispatch_event(event)
        return True

    def _render(self) -> None:
        self.router.draw_pages(self.screen)
        pygame.display.flip()

    def _run_event_loop(self, running: bool) -> bool:
        event = pygame.event.wait()
        if event.type == pygame.QUIT:
            return False
        self.router.dispatch_event(event)
        # 处理 wait 期间积压的事件
        for e in pygame.event.get():
            if e.type == pygame.QUIT:
                return False
            self.router.dispatch_event(e)
        self._render()
        return True

    def _run_fps_loop(self, running: bool) -> bool:
        if not self._process_events():
            return False
        self._render()
        self._clock.tick(self._target_fps)
        return True

    def _run_vsync_loop(self, running: bool) -> bool:
        """VSYNC：flip() 阻塞等待垂直同步，无需 clock.tick。"""
        if not self._process_events():
            return False
        self._render()
        return True
