#include "rune_cef_shim.h"

#include <atomic>
#include <mutex>
#include <string>
#include <vector>

#include "include/cef_app.h"
#include "include/cef_browser.h"
#include "include/cef_client.h"
#include "include/cef_render_handler.h"
#include "include/internal/cef_mac.h"

namespace {

std::string EncodeDataUrl(const char* html) {
    std::string out;
    out.reserve(std::strlen(html) * 2);
    out.append("data:text/html;charset=utf-8,");

    auto hex = [](unsigned char c) -> char {
        const char* digits = "0123456789ABCDEF";
        return digits[c & 0x0F];
    };

    for (const unsigned char c : std::string(html)) {
        switch (c) {
        case ' ':
            out.append("%20");
            break;
        case '<':
            out.append("%3C");
            break;
        case '>':
            out.append("%3E");
            break;
        case '#':
            out.append("%23");
            break;
        case '%':
            out.append("%25");
            break;
        case '"':
            out.append("%22");
            break;
        case '\n':
            out.append("%0A");
            break;
        case '\r':
            out.append("%0D");
            break;
        default:
            if ((c >= '0' && c <= '9') ||
                (c >= 'A' && c <= 'Z') ||
                (c >= 'a' && c <= 'z') ||
                c == '-' || c == '_' || c == '.' || c == '~') {
                out.push_back(static_cast<char>(c));
            } else {
                out.push_back('%');
                out.push_back(hex(c >> 4));
                out.push_back(hex(c));
            }
            break;
        }
    }
    return out;
}

struct RuneFrame {
    std::vector<uint8_t> pixels;
    uint32_t width = 0;
    uint32_t height = 0;
    uint32_t stride = 0;
};

class RuneClient : public CefClient,
                   public CefRenderHandler,
                   public CefLoadHandler {
public:
    explicit RuneClient(uint32_t width, uint32_t height, float scale_factor)
        : width_(width), height_(height), scale_factor_(scale_factor) {}

    CefRefPtr<CefRenderHandler> GetRenderHandler() override {
        return this;
    }

    CefRefPtr<CefLoadHandler> GetLoadHandler() override {
        return this;
    }

    void resize(uint32_t width, uint32_t height) {
        width_ = width;
        height_ = height;
    }

    void GetViewRect(CefRefPtr<CefBrowser>, CefRect& rect) override {
        rect.x = 0;
        rect.y = 0;
        rect.width = static_cast<int>(width_);
        rect.height = static_cast<int>(height_);
    }

    bool GetRootScreenRect(CefRefPtr<CefBrowser>, CefRect& rect) override {
        rect.x = 0;
        rect.y = 0;
        rect.width = static_cast<int>(width_);
        rect.height = static_cast<int>(height_);
        return true;
    }

    void OnPaint(CefRefPtr<CefBrowser>,
                 PaintElementType type,
                 const RectList&,
                 const void* buffer,
                 int width,
                 int height) override {
        if (type != PET_VIEW) {
            return;
        }

        const uint32_t w = static_cast<uint32_t>(width);
        const uint32_t h = static_cast<uint32_t>(height);
        const uint32_t stride = w * 4;
        const size_t size = static_cast<size_t>(stride) * static_cast<size_t>(h);

        std::lock_guard<std::mutex> lock(mutex_);
        frame_.width = w;
        frame_.height = h;
        frame_.stride = stride;
        frame_.pixels.resize(size);
        const uint8_t* src = static_cast<const uint8_t*>(buffer);
        std::copy(src, src + size, frame_.pixels.begin());
    }

    void OnLoadingStateChange(CefRefPtr<CefBrowser>,
                              bool isLoading,
                              bool,
                              bool) override {
        loading_.store(isLoading, std::memory_order_relaxed);
    }

    bool is_loading() const {
        return loading_.load(std::memory_order_relaxed);
    }

    bool get_frame(RuneFrame& out) {
        std::lock_guard<std::mutex> lock(mutex_);
        if (frame_.width == 0 || frame_.height == 0 || frame_.pixels.empty()) {
            return false;
        }
        out = frame_;
        return true;
    }

    uint32_t width() const {
        return width_;
    }

    uint32_t height() const {
        return height_;
    }

private:
    uint32_t width_;
    uint32_t height_;
    [[maybe_unused]] float scale_factor_;

    mutable std::mutex mutex_;
    RuneFrame frame_;
    std::atomic<bool> loading_{true};

    IMPLEMENT_REFCOUNTING(RuneClient);
    DISALLOW_COPY_AND_ASSIGN(RuneClient);
};

struct RuneBrowser {
    CefRefPtr<RuneClient> client;
    CefRefPtr<CefBrowser> browser;
};

std::atomic<bool> g_cef_initialized{false};

}  // namespace

int rune_cef_init(const char* cache_path,
                  const char* root_cache_path,
                  const char* log_file_path,
                  int external_message_pump) {
    if (g_cef_initialized.load(std::memory_order_acquire)) {
        return 1;
    }

    CefMainArgs main_args;
    CefRefPtr<CefApp> app;

    CefSettings settings;
    settings.no_sandbox = true;
    settings.windowless_rendering_enabled = true;
    settings.external_message_pump = external_message_pump ? true : false;
    settings.multi_threaded_message_loop = false;

    if (cache_path && *cache_path) {
        CefString(&settings.cache_path) = cache_path;
    }
    if (root_cache_path && *root_cache_path) {
        CefString(&settings.root_cache_path) = root_cache_path;
    }
    if (log_file_path && *log_file_path) {
        CefString(&settings.log_file) = log_file_path;
    }

    const bool ok = CefInitialize(main_args, settings, app, nullptr);
    if (!ok) {
        return 0;
    }

    g_cef_initialized.store(true, std::memory_order_release);
    return 1;
}

void rune_cef_shutdown(void) {
    if (!g_cef_initialized.load(std::memory_order_acquire)) {
        return;
    }
    CefShutdown();
    g_cef_initialized.store(false, std::memory_order_release);
}

rune_cef_browser_t rune_cef_create_browser(const rune_cef_config_t* config,
                                           const char* initial_url) {
    if (!g_cef_initialized.load(std::memory_order_acquire) || !config) {
        return nullptr;
    }

    CefWindowInfo window_info;
    window_info.SetAsWindowless(kNullWindowHandle);

    CefBrowserSettings browser_settings;
    browser_settings.windowless_frame_rate = 60;
    browser_settings.javascript = config->enable_javascript ? STATE_ENABLED : STATE_DISABLED;

    CefRefPtr<RuneClient> client =
        new RuneClient(config->width, config->height, config->scale_factor);

    CefString url = initial_url && *initial_url ? initial_url : "about:blank";

    CefRefPtr<CefBrowser> browser = CefBrowserHost::CreateBrowserSync(
        window_info, client, url, browser_settings, nullptr, nullptr);

    if (!browser) {
        return nullptr;
    }

    RuneBrowser* rb = new RuneBrowser;
    rb->client = client;
    rb->browser = browser;
    return static_cast<rune_cef_browser_t>(rb);
}

void rune_cef_destroy_browser(rune_cef_browser_t handle) {
    if (!handle) {
        return;
    }
    RuneBrowser* rb = static_cast<RuneBrowser*>(handle);
    if (rb->browser) {
        CefRefPtr<CefBrowserHost> host = rb->browser->GetHost();
        if (host) {
            host->CloseBrowser(true);
        }
        rb->browser = nullptr;
    }
    rb->client = nullptr;
    delete rb;
}

void rune_cef_navigate(rune_cef_browser_t handle, const char* url) {
    if (!handle || !url) {
        return;
    }
    RuneBrowser* rb = static_cast<RuneBrowser*>(handle);
    if (!rb->browser) {
        return;
    }
    CefRefPtr<CefFrame> frame = rb->browser->GetMainFrame();
    if (!frame) {
        return;
    }
    frame->LoadURL(url);
}

void rune_cef_load_html(rune_cef_browser_t handle,
                        const char* html,
                        const char* base_url) {
    if (!handle || !html) {
        return;
    }
    RuneBrowser* rb = static_cast<RuneBrowser*>(handle);
    if (!rb->browser) {
        return;
    }
    CefRefPtr<CefFrame> frame = rb->browser->GetMainFrame();
    if (!frame) {
        return;
    }

    if (!html) {
        return;
    }

    const std::string data_url = EncodeDataUrl(html);
    CefString url = data_url;
    frame->LoadURL(url);
}

void rune_cef_do_message_loop_work(void) {
    if (!g_cef_initialized.load(std::memory_order_acquire)) {
        return;
    }
    CefDoMessageLoopWork();
}

int rune_cef_is_loading(rune_cef_browser_t handle) {
    if (!handle) {
        return 0;
    }
    RuneBrowser* rb = static_cast<RuneBrowser*>(handle);
    if (!rb->client) {
        return 0;
    }
    return rb->client->is_loading() ? 1 : 0;
}

int rune_cef_get_frame(rune_cef_browser_t handle, rune_cef_frame_t* out_frame) {
    if (!handle || !out_frame) {
        return 0;
    }
    RuneBrowser* rb = static_cast<RuneBrowser*>(handle);
    if (!rb->client) {
        return 0;
    }
    RuneFrame frame;
    if (!rb->client->get_frame(frame)) {
        return 0;
    }

    // Copy into a stable buffer that outlives this function call.
    static thread_local std::vector<uint8_t> s_pixels;
    s_pixels = frame.pixels;

    out_frame->width = frame.width;
    out_frame->height = frame.height;
    out_frame->stride = frame.stride;
    out_frame->pixels = s_pixels.empty() ? nullptr : s_pixels.data();
    return 1;
}

void rune_cef_send_mouse_event(rune_cef_browser_t handle,
                               const rune_mouse_event_t* event) {
    if (!handle || !event) {
        return;
    }
    RuneBrowser* rb = static_cast<RuneBrowser*>(handle);
    if (!rb->browser) {
        return;
    }
    CefMouseEvent ev;
    ev.x = event->x;
    ev.y = event->y;
    ev.modifiers = static_cast<uint32_t>(event->modifiers);

    CefRefPtr<CefBrowserHost> host = rb->browser->GetHost();
    if (!host) {
        return;
    }

    switch (event->kind) {
    case RUNE_MOUSE_MOVE:
        host->SendMouseMoveEvent(ev, false);
        break;
    case RUNE_MOUSE_DOWN:
    case RUNE_MOUSE_UP: {
        CefBrowserHost::MouseButtonType btn = MBT_LEFT;
        switch (event->button) {
        case RUNE_MOUSE_LEFT:
            btn = MBT_LEFT;
            break;
        case RUNE_MOUSE_MIDDLE:
            btn = MBT_MIDDLE;
            break;
        case RUNE_MOUSE_RIGHT:
            btn = MBT_RIGHT;
            break;
        default:
            btn = MBT_LEFT;
            break;
        }
        const bool mouse_up = (event->kind == RUNE_MOUSE_UP);
        int click_count = 1;
        host->SendMouseClickEvent(ev, btn, mouse_up, click_count);
        break;
    }
    case RUNE_MOUSE_WHEEL:
        host->SendMouseWheelEvent(ev, event->delta_x, event->delta_y);
        break;
    }
}

void rune_cef_send_key_event(rune_cef_browser_t handle,
                             const rune_key_event_t* event) {
    if (!handle || !event) {
        return;
    }
    RuneBrowser* rb = static_cast<RuneBrowser*>(handle);
    if (!rb->browser) {
        return;
    }
    CefRefPtr<CefBrowserHost> host = rb->browser->GetHost();
    if (!host) {
        return;
    }

    CefKeyEvent ev;
    ev.windows_key_code = static_cast<int>(event->key_code);
    ev.native_key_code = static_cast<int>(event->key_code);
    ev.modifiers = static_cast<uint32_t>(event->modifiers);

    switch (event->kind) {
    case RUNE_KEY_DOWN:
        ev.type = KEYEVENT_RAWKEYDOWN;
        break;
    case RUNE_KEY_UP:
        ev.type = KEYEVENT_KEYUP;
        break;
    case RUNE_KEY_CHAR:
        ev.type = KEYEVENT_CHAR;
        ev.character = static_cast<char16_t>(event->character);
        ev.unmodified_character = static_cast<char16_t>(event->character);
        break;
    }

    host->SendKeyEvent(ev);
}

void rune_cef_resize(rune_cef_browser_t handle, uint32_t width, uint32_t height) {
    if (!handle) {
        return;
    }
    RuneBrowser* rb = static_cast<RuneBrowser*>(handle);
    if (!rb->browser || !rb->client) {
        return;
    }

    rb->client->resize(width, height);

    CefRefPtr<CefBrowserHost> host = rb->browser->GetHost();
    if (host) {
        host->WasResized();
    }
}
