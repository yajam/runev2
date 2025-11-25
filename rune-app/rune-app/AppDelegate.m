#import "AppDelegate.h"
#import "ViewController.h"
#import <Cocoa/Cocoa.h>

@implementation AppDelegate

bool finishedLaunching;
bool contextInitialized;

- (void)nextInitializationStep {
    @synchronized(self){
        if (finishedLaunching && contextInitialized) {
            NSStoryboard *storyBoard = [NSStoryboard storyboardWithName:@"Main" bundle:nil];
            NSWindowController *windowController = [storyBoard instantiateInitialController];
            [windowController showWindow:self];

            // Resize initial window to fill the visible bounds of its screen.
            NSWindow *window = windowController.window;
            if (window) {
                NSScreen *screen = window.screen ?: [NSScreen mainScreen];
                if (screen) {
                    NSRect visibleFrame = screen.visibleFrame;
                    [window setFrame:visibleFrame display:YES];

                    // Enforce a reasonable minimum content size so the IR layout
                    // doesn't collapse to extremely narrow widths. This matches
                    // the behavior of browsers like Safari that prevent the
                    // window from shrinking beyond a practical viewport width.
                    // Width clamp reduces layout thrash and "micro shrink/expand"
                    // artifacts during rapid resizing.
                    // Allow the window to shrink down to a narrow mobile-like
                    // width while still keeping a reasonable minimum height.
                    NSSize minSize = NSMakeSize(480.0, 600.0);
                    window.contentMinSize = minSize;
                }
            }
        }
    }
}

- (void)cefContextInitialized {
    contextInitialized = true;
    [self nextInitializationStep];
   
}

- (void)applicationDidFinishLaunching:(NSNotification *)aNotification {
    finishedLaunching = true;
    [self nextInitializationStep];
}

- (void)applicationWillTerminate:(NSNotification *)aNotification {
    // Insert code here to tear down your application
}


@end
