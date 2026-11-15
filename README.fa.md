<div dir="rtl">

این ترجمهٔ فارسی فایل [README.md](README.md) است. فایل انگلیسی [README.md](README.md) مرجع اصلی است.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brand/logo/kiri-horizontal-dark.svg">
    <img src="brand/logo/kiri-horizontal-light.svg" alt="Kiri" width="220">
  </picture>
</p>

<p align="center">
  <b>یک محیط اجرای کانتینرِ بسیار کوچک و سریع برای macOS، Windows و Linux.</b><br>
  ایمیج‌های Docker شما را اجرا می‌کند و با <code>docker</code> CLI، Compose و buildx که از قبل استفاده می‌کنید کار می‌کند.
</p>

<p align="center">
  <a href="https://runkiri.com/releases/">دانلود</a> ·
  <a href="docs/features.md">ویژگی‌ها</a> ·
  <a href="docs/development.md">ساخت از سورس</a> ·
  <a href="https://github.com/libracoder/kiri/releases">نسخه‌ها</a>
</p>

<p align="center">
  <a href="https://github.com/libracoder/kiri/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/libracoder/kiri?label=release&color=2f6feb"></a>
  <a href="LICENSE"><img alt="License: Apache-2.0" src="https://img.shields.io/badge/license-Apache--2.0-555"></a>
  <a href="#in-your-language"><img alt="38 languages" src="https://img.shields.io/badge/languages-38-2ea043"></a>
</p>

<p align="center">
  🇬🇧 🇮🇩 🇩🇪 🇪🇸 🇫🇷 🇮🇹 🇵🇱 🇧🇷 🇻🇳 🇹🇷 🇷🇺 🇸🇦 🇯🇵 🇰🇷 🇨🇳 🇹🇼<br>
  <sub>Kiri به 38 زبان صحبت می‌کند. <a href="#in-your-language">زبان شما هم یکی از آن‌هاست؟</a></sub>
</p>

<p align="center">
  <img src="docs/images/kiri-window.png" alt="پنجرهٔ Kiri که کانتینرها را بر اساس پروژهٔ Compose با وضعیت، پورت‌ها، دامنه‌های محلی و اندازه نشان می‌دهد" width="900">
</p>

---

Kiri یک جایگزین سبک برای Docker Desktop و OrbStack است که با زبان Rust نوشته شده است. کانتینرها را در یک ماشین مجازی کوچک Linux اجرا می‌کند (یا مستقیم روی کرنل، در Linux) و با API موتور Docker کار می‌کند، پس ابزارهای شما بدون تغییر کار می‌کنند.

نام Kiri از کلمهٔ ایگبوی *ntakịrị* به معنای «کوچک» گرفته شده است.

## چرا Kiri

- **سریع**: یک کانتینر از طریق API داکر در حدود 2.5 ms شروع و خارج می‌شود. این سرعت حدود 80 برابر Docker Desktop است.
- **سبک**: ماشین مجازی پس از 60 s بیکاری به خواب می‌رود، سرویس پس‌زمینه (daemon) حدود 15 MB حافظه مصرف می‌کند و در حدود 0.15 s بیدار می‌شود.
- **سازگار**: `docker`، Compose، buildx، `docker login` و ایمیج‌های amd64 از طریق Rosetta.
- **دامنه‌های محلی**: `http://<name>.kiri.local`، با HTTPS از مرجع صدور گواهی خود Kiri.
- **بیش از کانتینر**: یک فرمان برای راه‌اندازی یک کلاستر Kubernetes و ماشین‌های Linux.
- **یک پنجرهٔ بومی** برای کانتینرها، ایمیج‌ها، والیوم‌ها و شبکه‌ها، به‌همراه یک آیتم در نوار منو.
- **به زبان خودتان**: 38 زبان، روی macOS، Windows و Linux به یک شکل.

<a id="in-your-language"></a>
### به زبان خودتان

پنجره و راه‌اندازی Kiri به این زبان‌ها موجود است. Kiri از زبان سیستم شما پیروی می‌کند و می‌توانید آن را در تنظیمات ← ظاهر تغییر دهید. در عربی، فارسی و کوردیِ ناوەندی، کل پنجره از راست به چپ نمایش داده می‌شود.

| | Language | | Language | | Language | | Language |
|---|---|---|---|---|---|---|---|
| 🇬🇧 | English | 🇳🇴 | Norsk bokmål | 🇧🇬 | Български | 🇮🇳 | తెలుగు |
| 🇮🇩 | Bahasa Indonesia | 🇵🇱 | Polski | 🇷🇺 | Русский | 🇹🇭 | ไทย |
| 🇦🇩 | Català | 🇧🇷 | Português (Brasil) | 🇺🇦 | Українська | 🇱🇦 | ລາວ |
| 🇨🇿 | Čeština | 🇷🇴 | Română | 🇰🇿 | Қазақша | 🇯🇵 | 日本語 |
| 🇩🇪 | Deutsch | 🇦🇱 | Shqip | 🇸🇦 | العربية | 🇰🇷 | 한국어 |
| 🇪🇸 | Español | 🇸🇮 | Slovenščina | 🇮🇷 | فارسی | 🇨🇳 | 简体中文 |
| 🇫🇷 | Français | 🇫🇮 | Suomi | 🇮🇶 | کوردیی ناوەندی | 🇹🇼 | 繁體中文 |
| 🇮🇹 | Italiano | 🇻🇳 | Tiếng Việt | 🇮🇳 | हिन्दी | 🇪🇹 | አማርኛ |
| 🇱🇹 | Lietuvių | 🇹🇷 | Türkçe | 🇧🇩 | বাংলা |  |  |
| 🇭🇺 | Magyar | 🇬🇷 | Ελληνικά | 🇮🇳 | தமிழ் |  |  |

زبان شما در این فهرست نیست؟ ترجمه‌ها در [`crates/kiri/src/ui/lang`](crates/kiri/src/ui/lang) قرار دارند؛ به بخش [مشارکت](#contributing) مراجعه کنید.

## نصب

| پلتفرم | دریافت |
|----------|--------|
| macOS 14+ (Apple silicon یا Intel) | فایل `.dmg` از [runkiri.com/releases](https://runkiri.com/releases/) |
| Windows 10 و 11 (x86-64 یا Arm، از طریق WSL 2) | فایل `.msi` از [runkiri.com/releases](https://runkiri.com/releases/) |
| Linux (Ubuntu، Fedora و دیگران) | فایل `.deb` یا `.rpm` از [runkiri.com/releases](https://runkiri.com/releases/) |

بار اول که Kiri را باز می‌کنید، یک context با نام `kiri` برای Docker راه‌اندازی می‌کند. Kiri خودش را به‌روز می‌کند و امضای هر نسخهٔ جدید را پیش از نصب بررسی می‌کند.

## شروع سریع

</div>

```sh
docker --context kiri run --rm alpine echo hello
docker --context kiri compose up -d
kiri k8s start                      # single-node Kubernetes as the `kiri` kubectl context
kiri machine create dev --distro ubuntu && kiri machine shell dev
kiri import                         # bring images, volumes and containers over from OrbStack or Docker Desktop
```

<div dir="rtl">

دستورهای Docker به‌صورت فرمان‌های `kiri` هم کار می‌کنند (`kiri run`، `kiri ps`، `kiri compose up` و غیره) و همیشه به Kiri هدف می‌گیرند.

## عددها

`alpine` در حالی که `true` را اجرا و سپس حذف می‌شود، میانهٔ 30 اجرا، با همان `docker` CLI برای هر سه:

|                | Docker Engine API | `docker` CLI |
|----------------|------------------:|-------------:|
| **Kiri**       | **2.55 ms**       | **14.2 ms**  |
| Docker Desktop | 213 ms            | 243 ms       |
| OrbStack       | 380 ms            | 442 ms       |

روش اندازه‌گیری و همهٔ ارقام دیگر در [`bench/results.json`](bench/results.json) است. برای اجرای دوباره، از `kiri bench` استفاده کنید.

## نحوهٔ کار

</div>

```
docker CLI ─┐
compose ────┼─ Docker Engine API ─► kiri daemon ── vsock ──► guest agent (PID 1)
buildx ─────┤                         owns the VM           clone3 → container
kiri UI ────┘
```

<div dir="rtl">

ماشین مجازی از یک کرنل سفارشی 10 MB استفاده می‌کند و PID 1 آن یک عامل Rust حدود 900 KB است. این عامل کانتینرها را با یک `clone3` واحد در فضاهای نام (namespace) و یک cgroup راه‌اندازی می‌کند. در آن runc، shim یا dockerd وجود ندارد. جزئیات بیشتر در [docs/development.md](docs/development.md#architecture) آمده است.

## مستندات

- [ویژگی‌ها](docs/features.md): همهٔ مواردِ پشتیبانی‌شدهٔ امروز، فایل‌های انتشار و مشکلات شناخته‌شده
- [توسعه](docs/development.md): ساخت، اجرا از سورس، معماری و ساختار مخزن
- [یادداشت‌های انتشار](docs/releases)

## نقشهٔ راه

مرحلهٔ بعدی یک سازندهٔ داخلی (built-in builder) است. Kiri نقطه‌های پایانی (endpoint) ساخت Docker را ارائه می‌دهد، تا buildx از درایور docker آن استفاده کند، همان‌طور که در Docker Desktop استفاده می‌کند. این کار پشتیبانی از `host-gateway` را برای ساخت‌های BuildKit می‌آورد و ایمیج‌ها مستقیم در Kiri بارگذاری می‌شوند.

<a id="contributing"></a>
## مشارکت

مسئله‌ها (issues) و درخواست‌های pull خوش‌آمد هستند. به [CONTRIBUTING.md](CONTRIBUTING.md) و [آیین‌نامهٔ رفتار](CODE_OF_CONDUCT.md) مراجعه کنید. برای گزارش خصوصی یک آسیب‌پذیری، به [SECURITY.md](SECURITY.md) مراجعه کنید.

## مجوز

[Apache-2.0](LICENSE). مجوزهای اجزای شخص ثالث در [NOTICE](NOTICE) آمده است. کرنل مهمان Linux است (GPL-2.0)؛ پیکربندی، وصله‌ها و پیشنهاد منبع آن در [`guest/kernel`](guest/kernel) قرار دارند.

نام «Kiri» و لوگوی Kiri تحت پوشش مجوز نیستند. فورک‌ها، با نام و لوگوی دیگر، خوش‌آمد هستند.

</div>
