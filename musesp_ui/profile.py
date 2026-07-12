"""性能分析装饰器。标记 ``@profile`` 即可累计函数耗时，退出时自动打印。

受 ``config.debug.ui.render_profile`` 控制：
- ``true``  — 启用计时与退出打印
- ``false`` — ``@profile`` 退化为无操作，零运行时开销

跟踪两种时间：
- **Self**:  函数自身耗时（排除内部其他 @profile 函数的耗时）
- **Total**: 函数 + 所有子调用的 Wall-clock 耗时
"""

import atexit
import functools
import threading
import time
from collections import defaultdict

from musesp_config.config import config

if config.debug.ui.render_profile:

    _self_data: dict[str, list[float]] = defaultdict(list)
    _total_data: dict[str, list[float]] = defaultdict(list)
    _tls = threading.local()

    def _get_stack() -> list[list]:
        """获取当前线程的调用栈（惰性初始化）。"""
        s = getattr(_tls, "stack", None)
        if s is None:
            s = []
            _tls.stack = s
        return s

    def profile(func):
        @functools.wraps(func)
        def wrapper(*args, **kwargs):
            name = func.__qualname__
            start = time.perf_counter()
            stack = _get_stack()
            stack.append([name, start, 0.0])
            try:
                return func(*args, **kwargs)
            finally:
                elapsed = time.perf_counter() - start
                _, _, child_time = stack.pop()

                self_time = elapsed - child_time

                entry = _self_data[name]
                if not entry:
                    entry.extend([0.0, 0])
                entry[0] += self_time
                entry[1] += 1

                tentry = _total_data[name]
                if not tentry:
                    tentry.extend([0.0, 0])
                tentry[0] += elapsed
                tentry[1] += 1

                if stack:
                    stack[-1][2] += elapsed
        return wrapper

    def _print_profile() -> None:
        if not _self_data:
            return
        sorted_items = sorted(
            _self_data.items(), key=lambda kv: kv[1][0], reverse=True)
        print("\n" + "=" * 80)
        print("  PROFILE  (sorted by Self time)")
        print("=" * 80)
        print(f"  {'Function':<48} {'Self(ms)':>9} {'Total(ms)':>9} {'Calls':>7}")
        print("  " + "-" * 74)
        for name, (self_t, calls) in sorted_items:
            total_t = _total_data.get(name, [0.0, 0])[0]
            print(f"  {name:<48} {self_t * 1000:>9.1f} {total_t * 1000:>9.1f} {calls:>7}")
        print("=" * 80)

    atexit.register(_print_profile)

else:
    # 禁用时 profile 为恒等函数，零开销
    def profile(func):
        return func
