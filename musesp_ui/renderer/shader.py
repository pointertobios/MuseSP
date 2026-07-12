"""着色器基类。

========== 数据格式 ==========

VBO
    形状 ``(N, D_in)`` 的 float32 二维数组，N 个顶点，每行 D_in 个浮点数。
    前 4 列通常为 position (x, y, z, w)，后续列为自定义属性（颜色、法线、UV 等）。
    行列布局由调用方和具体 Shader 子类约定一致即可。

IBO
    形状 ``(M,)`` 的 int32 一维数组，每 3 个元素构成一个三角形。
    M 必须是 3 的倍数。值域 [0, N-1]，索引 VBO 中的顶点。

========== Shader 子类 ==========

继承 ``Shader`` 并实现 ``vertex()`` 和 ``fragment()``：

.. code-block:: python

    class MyShader(Shader):
        def vertex(self, vbo):
            # vbo: (N, 7) float32，列含义 [px, py, pz, pw, r, g, b]
            # 返回 (N, D_out) float32
            #      [:, :4] = NDC position (x, y, z, w)
            #      [:, 4:] = 任意 varyings（传入 fragment）
            return vbo  # 直通

        def fragment(self, varyings):
            # varyings: 插值后的属性，不含 position
            #           shape (D_out - 4,)
            # 返回 (r, g, b)，0-255
            r, g, b = varyings[0], varyings[1], varyings[2]
            return (int(r * 255), int(g * 255), int(b * 255))

========== 光栅化器 ==========

可覆盖 ``rasterize(v0, v1, v2, fb, db)`` 自定义光栅化策略。
默认实现：

- 接收三个顶点的屏幕空间数据，每顶点为一个 float64 一维数组：
  ``[screen_x, screen_y, screen_z, attr0, attr1, ...]``
- 计算三角形包围盒，逐像素重心坐标判断覆盖
- 重心坐标插值 varyings，传给 ``fragment()``
- 深度测试写入 framebuffer

========== 示例 ==========

.. code-block:: python

    from musesp_ui.renderer.shader import Shader

    class SimpleShader(Shader):
        def vertex(self, vbo):
            return vbo

        def fragment(self, varyings):
            r, g, b = varyings
            return (int(r * 255), int(g * 255), int(b * 255))

    # 一个彩色三角形
    vbo = np.array([
        [ 0.0,  0.5, 0.0, 1.0,  1.0, 0.0, 0.0],
        [-0.5, -0.5, 0.0, 1.0,  0.0, 1.0, 0.0],
        [ 0.5, -0.5, 0.0, 1.0,  0.0, 0.0, 1.0],
    ], dtype=np.float32)
    ibo = np.array([0, 1, 2], dtype=np.int32)

    rc = RendererCanvas(SimpleShader(), vbo, ibo, width=400, height=300)
"""

import numpy as np

from musesp_ui.profile import profile


class Shader:

    def vertex(self, vbo: np.ndarray) -> np.ndarray:
        """顶点着色器。

        :param vbo: (N, D_in) float32
        :return: (N, D_out) float32，前 4 列为 NDC position (x,y,z,w)
        """
        raise NotImplementedError

    def fragment(self, varyings: np.ndarray) -> tuple[int, int, int, int]:
        """片元着色器。

        :param varyings: 插值后的属性（不含 position）
        :return: (r, g, b, a)，0-255
        """
        raise NotImplementedError

    def __init__(self):
        self._px_full: np.ndarray | None = None
        self._buf_w: int = 0

    def _ensure_px(self, w: int) -> None:
        """确保 _px_full 覆盖屏幕宽度。"""
        if self._buf_w >= w:
            return
        self._px_full = np.arange(w, dtype=np.float32) + 0.5
        self._buf_w = w

    @profile
    def rasterize(self, v0, v1, v2, fb, db):
        """分块并行光栅化器。"""
        TILE_H = 32
        setup = self._rasterize_setup(v0, v1, v2, fb)
        if setup is None:
            return

        y_starts = np.arange(setup["y_min"], setup["y_max"] + 1, TILE_H,
                             dtype=np.int32)
        if len(y_starts) == 1:
            self._rasterize_tile(int(y_starts[0]),
                                 min(int(y_starts[0]) +
                                     TILE_H, setup["y_max"] + 1),
                                 fb, setup)
            return

        from musesp_ui.pool import get_pool
        pool = get_pool()
        list(pool.map(
            lambda i: self._rasterize_tile(
                int(y_starts[i]),
                min(int(y_starts[i]) + TILE_H, setup["y_max"] + 1),
                fb, setup),
            range(len(y_starts)),
        ))

    def _rasterize_setup(self, v0, v1, v2, fb):
        """预处理：包围盒、面积、边缘系数、fragment 预计算。"""
        h, w = fb.shape[:2]
        self._ensure_px(w)

        p0, p1, p2 = v0[:2], v1[:2], v2[:2]

        x_min = max(0, int(min(p0[0], p1[0], p2[0])))
        x_max = min(w - 1, int(max(p0[0], p1[0], p2[0])))
        y_min = max(0, int(min(p0[1], p1[1], p2[1])))
        y_max = min(h - 1, int(max(p0[1], p1[1], p2[1])))
        if x_max < x_min or y_max < y_min:
            return None

        area = _edge(p0, p1, p2)
        if abs(area) < 1e-8:
            return None
        inv_area = np.float32(1.0 / area)

        A0, B0, C0 = _edge_coeffs(p1, p2)
        A1, B1, C1 = _edge_coeffs(p2, p0)
        A2, B2, C2 = _edge_coeffs(p0, p1)

        bb_w = x_max - x_min + 1
        px = self._px_full[x_min:x_max + 1]

        a0, a1, a2 = v0[3:], v1[3:], v2[3:]
        const_varyings = np.array_equal(a0, a1) and np.array_equal(a0, a2)

        src = None
        alpha = inv_alpha = 0.0
        if const_varyings:
            r, g, b, a_frag = self.fragment(a0)
            src = np.array([r, g, b, a_frag], dtype=np.float32)
            alpha = src[3] / 255.0
            inv_alpha = 1.0 - alpha

        return {
            "x_min": x_min, "bb_w": bb_w, "y_min": y_min, "y_max": y_max,
            "inv_area": inv_area,
            "A0_px": A0 * px, "B0": B0, "C0": C0,
            "A1_px": A1 * px, "B1": B1, "C1": C1,
            "A2_px": A2 * px, "B2": B2, "C2": C2,
            "a0": a0, "a1": a1, "a2": a2,
            "const_varyings": const_varyings,
            "src": src, "alpha": alpha, "inv_alpha": inv_alpha,
        }

    @profile
    def _rasterize_tile(self, y0, y1, fb, s):
        """处理一块：边计算 → 混合。"""
        w0, w1, w2 = self._rasterize_edges(y0, y1, s)
        self._rasterize_blend(w0, w1, w2, y0, fb, s)

    @profile
    def _rasterize_edges(self, y0, y1, s):
        """计算一块的边缘函数 w0/w1/w2。"""
        tile_h = y1 - y0
        bb_w = s["bb_w"]
        py = np.arange(y0, y1, dtype=np.float32) + 0.5

        w0 = np.empty((tile_h, bb_w), dtype=np.float32)
        w1 = np.empty((tile_h, bb_w), dtype=np.float32)
        w2 = np.empty((tile_h, bb_w), dtype=np.float32)

        np.add(np.broadcast_to(s["A0_px"][None, :], (tile_h, bb_w)),
               (s["B0"] * py + s["C0"])[:, None], out=w0)
        np.add(np.broadcast_to(s["A1_px"][None, :], (tile_h, bb_w)),
               (s["B1"] * py + s["C1"])[:, None], out=w1)
        np.add(np.broadcast_to(s["A2_px"][None, :], (tile_h, bb_w)),
               (s["B2"] * py + s["C2"])[:, None], out=w2)
        return w0, w1, w2

    @profile
    def _rasterize_blend(self, w0, w1, w2, y0, fb, s):
        """mask 计算 + 分发到 const/varying 路径。"""
        mask = ((w0 >= 0) & (w1 >= 0) & (w2 >= 0)) | (
            (w0 <= 0) & (w1 <= 0) & (w2 <= 0))
        if not np.any(mask):
            return

        y1 = y0 + w0.shape[0]
        fb_tile = fb[y0:y1, s["x_min"]:s["x_min"] + s["bb_w"]]

        if s["const_varyings"]:
            self._rasterize_blend_const(mask, fb_tile, s)
        else:
            self._rasterize_blend_varying(mask, fb_tile, w0, w1, w2, s)

    @profile
    def _rasterize_blend_const(self, mask, fb_tile, s):
        """常量 varyings 快速混合路径。"""
        src = s["src"]
        alpha = s["alpha"]
        inv_alpha = s["inv_alpha"]
        pixels = fb_tile[mask].astype(np.float32)
        pixels[:, 0] = src[0] * alpha + pixels[:, 0] * inv_alpha
        pixels[:, 1] = src[1] * alpha + pixels[:, 1] * inv_alpha
        pixels[:, 2] = src[2] * alpha + pixels[:, 2] * inv_alpha
        pixels[:, 3] = np.maximum(src[3], pixels[:, 3])
        fb_tile[mask] = pixels.astype(np.uint8)

    @profile
    def _rasterize_blend_varying(self, mask, fb_tile, w0, w1, w2, s):
        """逐像素 varyings 插值 + fragment + 混合路径。"""
        bc0 = w0[mask] * s["inv_area"]
        bc1 = w1[mask] * s["inv_area"]
        bc2 = w2[mask] * s["inv_area"]
        a0, a1, a2 = s["a0"], s["a1"], s["a2"]
        n_vis = len(bc0)
        colors = np.empty((n_vis, 4), dtype=np.uint8)
        for i in range(n_vis):
            vary = bc0[i] * a0 + bc1[i] * a1 + bc2[i] * a2
            cr, cg, cb, ca = self.fragment(vary)
            colors[i] = [cr, cg, cb, ca]
        alpha_a = colors[:, 3].astype(np.float32) / 255.0
        pixels = fb_tile[mask].astype(np.float32)
        pixels[:, 0] = colors[:, 0] * alpha_a + pixels[:, 0] * (1.0 - alpha_a)
        pixels[:, 1] = colors[:, 1] * alpha_a + pixels[:, 1] * (1.0 - alpha_a)
        pixels[:, 2] = colors[:, 2] * alpha_a + pixels[:, 2] * (1.0 - alpha_a)
        pixels[:, 3] = np.maximum(colors[:, 3], pixels[:, 3])
        fb_tile[mask] = pixels.astype(np.uint8)


@profile
def _edge(a, b, c):
    """2D 边缘函数：(b - a) × (c - a)。"""
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])


@profile
def _edge_coeffs(a, b):
    """边缘函数 e(a,b,px,py) = A·px + B·py + C 的系数。"""
    A = np.float32(a[1] - b[1])
    B = np.float32(b[0] - a[0])
    C = np.float32((b[1] - a[1]) * a[0] - (b[0] - a[0]) * a[1])
    return A, B, C
