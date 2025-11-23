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
