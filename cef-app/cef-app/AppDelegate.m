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
