"""MusicList 页面 —— 音乐选择界面。

左侧滚动列表显示可用音乐（名称 + 作者），右侧显示选中音乐的详细信息。
"""

from pathlib import Path

import pygame
import tomllib

from musesp_ui.components.button import Button
from musesp_ui.components.core import Component, Constraintable, Direction
from musesp_ui.components.image import Image, ImageMode
from musesp_ui.components.image_button import ImageButton
from musesp_ui.components.label import Label
from musesp_ui.components.scroll_list import ScrollList
from musesp_ui.components.spacer import Spacer
from musesp_ui.application import RunMode
from musesp_ui.router import Page, Router
from musesp_config.config import config
from musesp_gameplay.gameplay_page import GameplayPage


class MusicListPage(Page):
    def full_shadow_promise(self) -> bool:
        return True

    def on_activate(self) -> None:
        self._router._app.set_mode(RunMode.EVENT)

    def build(self) -> None:
        self._router._app.set_mode(RunMode.EVENT)
        self._root.layout_direction = Direction.HORIZONTAL

        # --- 左上角返回按钮（NONE 约束，不参与布局）---
        back_btn = ImageButton("assets/ui/return_button.svg",
                               x=16, y=16, width=44, height=44)
        back_btn.h_constraint = Constraintable.NONE
        back_btn.v_constraint = Constraintable.NONE
        back_btn.bind_on_mouse_click(self._on_back)
        self._root.add_sub_component(back_btn)

        # --- 内容容器 ---
        content = Component()
        content.layout_direction = Direction.HORIZONTAL
        content.h_constraint = Constraintable.MAXIMUM
        content.v_constraint = Constraintable.MINIMUM

        # --- 左侧列表 ---
        left = Component()
        left.layout_direction = Direction.VERTICAL
        left.h_constraint = Constraintable.MINIMUM
        left.v_constraint = Constraintable.MINIMUM
        left.min_width = 320

        self._scroll_list = ScrollList(item_height=52)
        self._scroll_list.v_constraint = Constraintable.MAXIMUM
        self._scroll_list.h_constraint = Constraintable.MINIMUM
        self._scroll_list.min_width = 280
        self._scroll_list.bind_on_select(self._on_music_select)
        left.add_sub_component(self._scroll_list)

        content.add_sub_component(left)

        # --- 分隔 ---
        sep = Spacer(width=2)
        sep.h_constraint = Constraintable.MINIMUM
        sep.v_constraint = Constraintable.MINIMUM
        sep.min_width = 2
        content.add_sub_component(sep)

        # --- 右侧详情 ---
        self._detail = Component()
        self._detail.layout_direction = Direction.VERTICAL
        self._detail.h_constraint = Constraintable.MAXIMUM
        self._detail.v_constraint = Constraintable.MINIMUM

        # 顶部弹性空间
        self._spacer_top = Spacer()
        self._spacer_top.v_constraint = Constraintable.MAXIMUM
        self._detail.add_sub_component(self._spacer_top)

        # 封面图
        self._cover = Image("", h_mode=ImageMode.KEEP_RATE,
                            v_mode=ImageMode.COVER)
        self._cover.v_constraint = Constraintable.MINIMUM
        self._cover.h_constraint = Constraintable.MINIMUM
        self._cover.min_width = 200
        self._detail.add_sub_component(self._cover)

        # 小间距
        gap1 = Spacer()
        gap1.v_constraint = Constraintable.MINIMUM
        gap1.min_height = 8
        self._detail.add_sub_component(gap1)

        # 难度按钮行
        self._diff_row = Component()
        self._diff_row.layout_direction = Direction.HORIZONTAL
        self._diff_row.v_constraint = Constraintable.MINIMUM
        self._diff_row.h_constraint = Constraintable.MINIMUM
        self._diff_row.min_height = 44
        self._diff_btns: list[Button] = []
        self._detail.add_sub_component(self._diff_row)

        # 小间距
        gap2 = Spacer()
        gap2.v_constraint = Constraintable.MINIMUM
        gap2.min_height = 8
        self._detail.add_sub_component(gap2)

        # Play 按钮
        self._btn_play = Button("▶ Play")
        self._btn_play.v_constraint = Constraintable.MINIMUM
        self._btn_play.h_constraint = Constraintable.MAXIMUM
        self._btn_play.min_height = 44
        self._btn_play.min_width = 200
        self._btn_play.bind_on_mouse_click(self._on_play)
        self._detail.add_sub_component(self._btn_play)

        # 底部弹性空间
        self._spacer_bottom = Spacer()
        self._spacer_bottom.v_constraint = Constraintable.MAXIMUM
        self._detail.add_sub_component(self._spacer_bottom)

        content.add_sub_component(self._detail)

        # --- 两侧 Spacer ---
        self._spacer_left = Spacer()
        self._spacer_left.h_constraint = Constraintable.MAXIMUM
        self._spacer_left.v_constraint = Constraintable.MINIMUM
        self._root.add_sub_component(self._spacer_left)

        self._root.add_sub_component(content)

        self._spacer_right = Spacer()
        self._spacer_right.h_constraint = Constraintable.MAXIMUM
        self._spacer_right.v_constraint = Constraintable.MINIMUM
        self._root.add_sub_component(self._spacer_right)

        # 加载音乐列表
        self._music_sources: dict[str, Path] = {}
        self._load_list()

    def prepare_layout(self) -> None:
        cap = self._root.width * 2 // 11
        self._spacer_left.max_width = cap
        self._spacer_right.max_width = cap
        # 封面最小高度：除去固定元素（间距+按钮）后取 60%
        fixed = 8 + self._diff_row.min_height + 8 + self._btn_play.min_height
        available = max(0, self._detail.height - fixed)
        self._cover.min_height = max(300, available * 3 // 2)

    def _load_list(self) -> None:
        """从 music_assets_path 中读取所有 list.txt，填充列表。"""
        comps: list[Component] = []
        for path_str in config.gameplay.music_assets_path:
            base = Path(path_str)
            list_file = self._resolve_list_file(base)
            if list_file is None:
                continue
            try:
                for line in list_file.read_text(encoding="utf-8").splitlines():
                    line = line.strip()
                    if not line:
                        continue
                    parts = line.split("|")
                    if len(parts) >= 3:
                        subdir, name, author = parts[0], parts[1], parts[2]
                        item_id = f"{path_str}/{subdir}"
                        self._music_sources[item_id] = base / subdir
                        comp = Component(
                            width=280, height=self._scroll_list.item_height)
                        comp._item_id = item_id
                        comp._selected = False

                        name_lbl = Label(name, font_size=20,
                                         color=(255, 255, 255))
                        name_lbl.y = 4
                        name_lbl.height = 24
                        comp._name_lbl = name_lbl
                        comp.add_sub_component(name_lbl)

                        author_lbl = Label(author, font_size=14,
                                           color=(160, 160, 160))
                        author_lbl.y = 28
                        author_lbl.height = 20
                        comp._author_lbl = author_lbl
                        comp.add_sub_component(author_lbl)

                        comp.bind_on_mouse_enter(
                            lambda e, c=comp: self._on_item_enter(c))
                        comp.bind_on_mouse_exit(
                            lambda e, c=comp: self._on_item_exit(c))
                        comps.append(comp)
            except OSError:
                continue
        self._scroll_list.set_items(comps)
        self._selected_item: Component | None = None

    def _on_item_enter(self, comp: Component) -> bool:
        comp._name_lbl.font_size = 22
        comp._name_lbl.color = (255, 255, 160)
        comp._author_lbl.color = (200, 200, 200)
        return True

    def _on_item_exit(self, comp: Component) -> bool:
        if not comp._selected:
            comp._name_lbl.font_size = 20
            comp._name_lbl.color = (255, 255, 255)
            comp._author_lbl.color = (160, 160, 160)
        return True

    def _resolve_list_file(self, base: Path) -> Path | None:
        """获取 list.txt，支持目录和 zstd 压缩包。"""
        if base.is_dir():
            f = base / "list.txt"
            return f if f.exists() else None
        # TODO: zstd 压缩包支持
        return None

    def _on_music_select(self, item_id: str) -> None:
        """列表项被点击，加载 meta.toml 并更新右侧详情。"""
        src = self._music_sources.get(item_id)
        if src is None or not src.is_dir():
            return
        meta = self._load_meta(src)
        if meta is None:
            return
        music = meta.get("music", {})

        # 取消旧选中
        if self._selected_item is not None:
            self._selected_item._selected = False
            self._selected_item._name_lbl.font_size = 20
            self._selected_item._name_lbl.color = (255, 255, 255)
            self._selected_item._author_lbl.color = (160, 160, 160)
            self._selected_item = None

        # 标记新选中
        for child in self._scroll_list._sub_components:
            if getattr(child, "_item_id", "") == item_id:
                child._selected = True
                self._selected_item = child
                child._name_lbl.font_size = 22
                child._name_lbl.color = (255, 255, 100)
                child._author_lbl.color = (220, 220, 220)
                break

        # 封面
        cover_name = music.get("cover", "")
        if cover_name:
            self._cover.set_image(str(src / cover_name))

        # 难度按钮
        self._diff_row._sub_components.clear()
        levels = music.get("levels", {})
        sorted_levels = sorted(levels.items(), key=lambda kv: int(kv[0]))
        self._level_btns: list[Button] = []
        for i, (lv, _) in enumerate(sorted_levels):
            btn = Button(f"Lv.{lv}", width=80, height=36, font_size=16)
            btn.h_constraint = Constraintable.MAXIMUM
            btn.v_constraint = Constraintable.MINIMUM
            btn.min_height = 36
            btn.min_width = 70
            btn.bind_on_mouse_click(lambda e, b=btn: self._on_level_select(b))
            self._level_btns.append(btn)
            self._diff_row.add_sub_component(btn)
            if i < len(sorted_levels) - 1:
                g = Spacer(width=4)
                g.h_constraint = Constraintable.MINIMUM
                g.min_width = 4
                self._diff_row.add_sub_component(g)
        # 触发布局更新
        self._root.layout()

    def _on_level_select(self, clicked: Button) -> bool:
        self._selected_level = int(clicked.text.replace("Lv.", ""))
        for btn in self._level_btns:
            btn.enable()
        clicked.disable()
        return False

    def _on_play(self, event) -> bool:
        if not hasattr(self, "_selected_level"):
            return False
        self._router.clear_and_push(GameplayPage())
        return False

    def _load_meta(self, src: Path) -> dict | None:
        """加载音乐目录或压缩包中的 meta.toml。"""
        meta_file = src / "meta.toml" if src.is_dir() else None
        if meta_file is None or not meta_file.exists():
            return None
        try:
            return tomllib.loads(meta_file.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError):
            return None

    def dispatch_event(self, event) -> bool:
        if not super().dispatch_event(event):
            return False
        return True

    def _on_back(self, event) -> bool:
        from musesp.pages.home import HomePage
        self._router.pop_then_else(lambda: HomePage())
        return False
