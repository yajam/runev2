#import <Cocoa/Cocoa.h>

#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Weverything"
#import "include/cef_app.h"
#import "include/cef_browser.h"
#import "include/cef_client.h"
#import "include/wrapper/cef_library_loader.h"
#pragma clang diagnostic pop

#import "AppDelegate.h"

// Global exception handler to prevent crashes
void setupExceptionHandling() {
    NSSetUncaughtExceptionHandler([](NSException *exception) {
        NSLog(@"Uncaught exception: %@", exception);
        NSLog(@"Stack trace: %@", [exception callStackSymbols]);
    });

    // Handle signals for crashes
    signal(SIGABRT, [](int signal) {
        NSLog(@"SIGABRT received - app would have crashed");
    });
    signal(SIGSEGV, [](int signal) {
        NSLog(@"SIGSEGV received - segmentation fault");
    });
}

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

            // Smooth scrolling optimizations
            command_line->AppendSwitch("enable-smooth-scrolling");
            command_line->AppendSwitch("disable-threaded-scrolling");

            // GPU compositing for better performance
            command_line->AppendSwitch("enable-gpu-rasterization");
            command_line->AppendSwitch("enable-zero-copy");

            // Reduce input latency
            command_line->AppendSwitch("disable-gpu-vsync");

//            command_line->AppendSwitch("show-fps-counter");
//            command_line->AppendSwitch("disable-gpu");
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

// Create the application menu bar programmatically
void setupMenuBar(NSString* appName) {
    NSMenu* menuBar = [[NSMenu alloc] init];
    [NSApp setMainMenu:menuBar];

    // Application menu
    NSMenuItem* appMenuItem = [[NSMenuItem alloc] init];
    [menuBar addItem:appMenuItem];

    NSMenu* appMenu = [[NSMenu alloc] init];
    [appMenuItem setSubmenu:appMenu];

    // About
    NSMenuItem* aboutItem = [[NSMenuItem alloc] initWithTitle:[NSString stringWithFormat:@"About %@", appName]
                                                       action:@selector(orderFrontStandardAboutPanel:)
                                                keyEquivalent:@""];
    [appMenu addItem:aboutItem];
    [appMenu addItem:[NSMenuItem separatorItem]];

    // Hide
    NSMenuItem* hideItem = [[NSMenuItem alloc] initWithTitle:[NSString stringWithFormat:@"Hide %@", appName]
                                                      action:@selector(hide:)
                                               keyEquivalent:@"h"];
    [appMenu addItem:hideItem];

    // Hide Others
    NSMenuItem* hideOthersItem = [[NSMenuItem alloc] initWithTitle:@"Hide Others"
                                                            action:@selector(hideOtherApplications:)
                                                     keyEquivalent:@"h"];
    [hideOthersItem setKeyEquivalentModifierMask:NSEventModifierFlagCommand | NSEventModifierFlagOption];
    [appMenu addItem:hideOthersItem];

    // Show All
    NSMenuItem* showAllItem = [[NSMenuItem alloc] initWithTitle:@"Show All"
                                                         action:@selector(unhideAllApplications:)
                                                  keyEquivalent:@""];
    [appMenu addItem:showAllItem];
    [appMenu addItem:[NSMenuItem separatorItem]];

    // Quit
    NSMenuItem* quitItem = [[NSMenuItem alloc] initWithTitle:[NSString stringWithFormat:@"Quit %@", appName]
                                                      action:@selector(terminate:)
                                               keyEquivalent:@"q"];
    [appMenu addItem:quitItem];

    // File menu
    NSMenuItem* fileMenuItem = [[NSMenuItem alloc] init];
    [menuBar addItem:fileMenuItem];

    NSMenu* fileMenu = [[NSMenu alloc] initWithTitle:@"File"];
    [fileMenuItem setSubmenu:fileMenu];

    // New Window
    NSMenuItem* newWindowItem = [[NSMenuItem alloc] initWithTitle:@"New Window"
                                                           action:@selector(newDocument:)
                                                    keyEquivalent:@"n"];
    [fileMenu addItem:newWindowItem];

    // Close Window
    NSMenuItem* closeItem = [[NSMenuItem alloc] initWithTitle:@"Close Window"
                                                       action:@selector(performClose:)
                                                keyEquivalent:@"w"];
    [fileMenu addItem:closeItem];

    // Edit menu
    NSMenuItem* editMenuItem = [[NSMenuItem alloc] init];
    [menuBar addItem:editMenuItem];

    NSMenu* editMenu = [[NSMenu alloc] initWithTitle:@"Edit"];
    [editMenuItem setSubmenu:editMenu];

    // Undo
    NSMenuItem* undoItem = [[NSMenuItem alloc] initWithTitle:@"Undo"
                                                      action:@selector(undo:)
                                               keyEquivalent:@"z"];
    [editMenu addItem:undoItem];

    // Redo
    NSMenuItem* redoItem = [[NSMenuItem alloc] initWithTitle:@"Redo"
                                                      action:@selector(redo:)
                                               keyEquivalent:@"Z"];
    [editMenu addItem:redoItem];
    [editMenu addItem:[NSMenuItem separatorItem]];

    // Cut
    NSMenuItem* cutItem = [[NSMenuItem alloc] initWithTitle:@"Cut"
                                                     action:@selector(cut:)
                                              keyEquivalent:@"x"];
    [editMenu addItem:cutItem];

    // Copy
    NSMenuItem* copyItem = [[NSMenuItem alloc] initWithTitle:@"Copy"
                                                      action:@selector(copy:)
                                               keyEquivalent:@"c"];
    [editMenu addItem:copyItem];

    // Paste
    NSMenuItem* pasteItem = [[NSMenuItem alloc] initWithTitle:@"Paste"
                                                       action:@selector(paste:)
                                                keyEquivalent:@"v"];
    [editMenu addItem:pasteItem];

    // Select All
    NSMenuItem* selectAllItem = [[NSMenuItem alloc] initWithTitle:@"Select All"
                                                           action:@selector(selectAll:)
                                                    keyEquivalent:@"a"];
    [editMenu addItem:selectAllItem];

    // View menu
    NSMenuItem* viewMenuItem = [[NSMenuItem alloc] init];
    [menuBar addItem:viewMenuItem];

    NSMenu* viewMenu = [[NSMenu alloc] initWithTitle:@"View"];
    [viewMenuItem setSubmenu:viewMenu];

    // Enter Full Screen
    NSMenuItem* fullScreenItem = [[NSMenuItem alloc] initWithTitle:@"Enter Full Screen"
                                                            action:@selector(toggleFullScreen:)
                                                     keyEquivalent:@"f"];
    [fullScreenItem setKeyEquivalentModifierMask:NSEventModifierFlagCommand | NSEventModifierFlagControl];
    [viewMenu addItem:fullScreenItem];

    // Bookmarks menu
    NSMenuItem* bookmarksMenuItem = [[NSMenuItem alloc] init];
    [menuBar addItem:bookmarksMenuItem];

    NSMenu* bookmarksMenu = [[NSMenu alloc] initWithTitle:@"Bookmarks"];
    [bookmarksMenuItem setSubmenu:bookmarksMenu];

    // Add Bookmark (Cmd+D)
    NSMenuItem* addBookmarkItem = [[NSMenuItem alloc] initWithTitle:@"Add Bookmark"
                                                             action:@selector(addBookmark:)
                                                      keyEquivalent:@"d"];
    [bookmarksMenu addItem:addBookmarkItem];

    // Add New Tab item to File menu (Cmd+T)
    NSMenuItem* newTabItem = [[NSMenuItem alloc] initWithTitle:@"New Tab"
                                                        action:@selector(newTab:)
                                                 keyEquivalent:@"t"];
    [fileMenu insertItem:newTabItem atIndex:1]; // Insert after "New Window"

    // Window menu
    NSMenuItem* windowMenuItem = [[NSMenuItem alloc] init];
    [menuBar addItem:windowMenuItem];

    NSMenu* windowMenu = [[NSMenu alloc] initWithTitle:@"Window"];
    [windowMenuItem setSubmenu:windowMenu];
    [NSApp setWindowsMenu:windowMenu];

    // Minimize
    NSMenuItem* minimizeItem = [[NSMenuItem alloc] initWithTitle:@"Minimize"
                                                          action:@selector(performMiniaturize:)
                                                   keyEquivalent:@"m"];
    [windowMenu addItem:minimizeItem];

    // Zoom
    NSMenuItem* zoomItem = [[NSMenuItem alloc] initWithTitle:@"Zoom"
                                                      action:@selector(performZoom:)
                                               keyEquivalent:@""];
    [windowMenu addItem:zoomItem];
    [windowMenu addItem:[NSMenuItem separatorItem]];

    // Bring All to Front
    NSMenuItem* bringAllItem = [[NSMenuItem alloc] initWithTitle:@"Bring All to Front"
                                                          action:@selector(arrangeInFront:)
                                                   keyEquivalent:@""];
    [windowMenu addItem:bringAllItem];

    // Help menu
    NSMenuItem* helpMenuItem = [[NSMenuItem alloc] init];
    [menuBar addItem:helpMenuItem];

    NSMenu* helpMenu = [[NSMenu alloc] initWithTitle:@"Help"];
    [helpMenuItem setSubmenu:helpMenu];
    [NSApp setHelpMenu:helpMenu];
}

int main(int argc, const char * argv[]) {

    // Setup exception handling to catch crashes gracefully
    setupExceptionHandling();

    // Load the CEF framework library at runtime instead of linking directly
    // as required by the macOS sandbox implementation.
    CefScopedLibraryLoader library_loader;
    if (!library_loader.LoadInMain()) {
        return 1;
    }

    @autoreleasepool {

        [NSApplication sharedApplication];

        // Set working directory to Resources folder so relative paths work
        // (images/, fonts/, etc. are loaded from here)
        NSString* resourcePath = [[NSBundle mainBundle] resourcePath];
        if (resourcePath) {
            [[NSFileManager defaultManager] changeCurrentDirectoryPath:resourcePath];
            NSLog(@"Working directory set to: %@", resourcePath);
        }

        // Set app name and appearance
        NSString* appName = @"Rune";
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];

        // Setup the menu bar with standard menus and keyboard shortcuts
        setupMenuBar(appName);

        AppDelegate* appDelegate = [AppDelegate new];
        [NSApp setDelegate:appDelegate];

        CefMainArgs cefMainArgs;

        CefSettings cefSettings;
        // Using native NSView rendering (not OSR/windowless)
        cefSettings.windowless_rendering_enabled = false;
        cefSettings.no_sandbox = true;

        CefRefPtr<CefBrowserApp> cefBrowserApp = new CefBrowserApp([appDelegate](){
            [appDelegate cefContextInitialized];
        });

        CefInitialize(cefMainArgs, cefSettings, cefBrowserApp, nil);

        // Activate the app (bring to front, show in dock)
        [NSApp activateIgnoringOtherApps:YES];

        CefRunMessageLoop();

        CefShutdown();
    }

}
