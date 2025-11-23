#import <Cocoa/Cocoa.h>

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Weverything"
#import "include/cef_app.h"
#import "include/cef_browser.h"
#import "include/cef_client.h"
#import "include/wrapper/cef_library_loader.h"
#pragma clang diagnostic pop




#import "AppDelegate.h"

class CefBrowserApp : public CefApp, public CefBrowserProcessHandler {
public:
    CefBrowserApp(std::function<void()> onContextinitialized)
        : onContextinitialized(onContextinitialized)
    {
       
    }
    
    CefRefPtr<CefBrowserProcessHandler> GetBrowserProcessHandler() override {
        return this;
    }
    
    void OnBeforeCommandLineProcessing(
         const CefString& process_type,
         CefRefPtr<CefCommandLine> command_line
    ) override {
        
        if (process_type.empty()) {
            
            command_line->AppendSwitch("use-mock-keychain");
//            command_line->AppendSwitch("show-fps-counter");
//            command_line->AppendSwitch("disable-gpu");
//            command_line->AppendSwitch("disable-gpu-vsync");
//            command_line->AppendSwitch("disable-frame-rate-limit");
//            command_line->AppendSwitch("disable-gpu-compositing");
//            // Don't create a "GPUCache" directory when cache-path is unspecified.
//            command_line->AppendSwitch("disable-gpu-shader-disk-cache");
         
        }
    }
    
    void OnContextInitialized() override {
        this->onContextinitialized();
    }
    
private:
    std::function<void()> onContextinitialized;
    
    IMPLEMENT_REFCOUNTING(CefBrowserApp);
    DISALLOW_COPY_AND_ASSIGN(CefBrowserApp);
};

int main(int argc, const char * argv[]) {
    
    // Load the CEF framework library at runtime instead of linking directly
    // as required by the macOS sandbox implementation.
    CefScopedLibraryLoader library_loader;
    if (!library_loader.LoadInMain()) {
        return 1;
    }
    
    @autoreleasepool {
        
        [NSApplication sharedApplication];
        
        AppDelegate* appDelegate = [AppDelegate new];
        [NSApp setDelegate:appDelegate];
        
        CefMainArgs cefMainArgs;
        
        CefSettings cefSettings;
        cefSettings.windowless_rendering_enabled = true;
        cefSettings.no_sandbox = true;
        
        CefRefPtr<CefBrowserApp> cefBrowserApp = new CefBrowserApp([appDelegate](){
            [appDelegate cefContextInitialized];
        });
      
        CefInitialize(cefMainArgs, cefSettings, cefBrowserApp, nil);
        
        CefRunMessageLoop();
        
        CefShutdown();
    }
   
}
