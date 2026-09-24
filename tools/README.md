# slot 定制工具集（66-mods 分支）

本目录收的是给 slot 前端做定制 / 真机验证 / 发布时用的小工具，按用途分四个子目录。
脚本里大多写死了本机工作区绝对路径（`F:/WorkBuddyWorkSpace/2026-09-11-08-38-21/...`），
clone 到别处按需改路径即可；它们都是离线脚本，不依赖网络。

## device/ —— 真机「看得见 + 按得动」
- `inject.sh`：往 `/dev/input/event1` 写 24 字节 `input_event`，模拟按键 / 方向（HAT）。
  用法 `sh inject.sh <seq> /dev/input/event1`，`<seq>` 见脚本内 `a_down/a_up/b_down/.../menu_down/menu_up/hat_left/...`。
  ⚠️ **凡是靠 down→up 间隔阈值判定的「轻点」手势（如 MENU 开关于 250ms 窗口），必须把 down 和 up 放进同一条 adb shell、
  用设备内 `sleep 0.05` 控制间隔**，否则宿主机 adb 往返延迟会吃掉时间窗、被当成长按。
  推到卡上 `/mnt/sdcard/inject.sh` 再 `adb shell "/mnt/sdcard/inject.sh ..."` 执行（卡上可执行，tmpfs 的 /tmp 重启即没）。
- `fb2png.py`：把 `cat /dev/fb0` 抓下来的 raw（720 宽 stride 2880、virtual 720×960 双缓冲）转成 PNG。
  `PIL.Image.frombytes("RGBA",(720,480),raw,"raw","BGRA")`。

## font/ —— 卡上字体子集
- `subset_font.py`：扫描 `crates/**/*.rs` 字符串字面量里的非 ASCII 字 + BASE_RANGES，从全量 Noto 子集出卡上字体。
- `augment_font.py`：**安全增补**配方 —— 新子集 = `当前卡上字体全部码点(A)` ∪ `BASE_RANGES + 源码非 ASCII 字(B)`，
  保证对已在售字体**零字形丢失（lost=0）**的同时补上源码新引入的字（如 `夹` U+5939）。
  用来修「缺字被静默渲染成空白」这类问题，不依赖「源码扫到没扫到」。

## release/ —— 发布件
- `pack.py`：把 `deploy/System/` 打成顶层为 `System/` 的 zip（可执行位 0755、固定时间戳），用于出前端发布包。
- `make_single.py`：把 `操作指南-图文版.html` 里的 `<img src="img/...">` 换成 `data:image/png;base64,...`，
  生成零外链的单文件 HTML（便于分发）。

## savetype/ —— GBA 存档类型预判
对整库 ROM 扫 5 个核心签名串 + 读 ROM 头 + 算 zlib crc32，按 mGBA / gpSP 各自规则预判存档类型，
定位「部分汉化 / 改版存不了」的根因（多签名、无签名、跨版本配对）。
- `scan.py`：主扫描；`analyse.py` / `pair.py`：归因；`elfver.py` / `elfcheck.py`：读 ELF 的 `.gnu.version_r`（verneed）判定 GLIBC 上限。
- `mgba-overrides.c`：mGBA 内建游戏码覆盖表（参考，解释为什么有些游戏被强制 FLASH1M+RTC）。
- `scan-result.csv`：930 款整合包的扫描明细（实测数据）。
