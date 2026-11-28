// UI-F5 wave-F close: XTEST injector for the export-panel capture.
// Supports: move X Y, click X Y, wheel X Y up|down [ticks].
// (Unchanged from the UI-F3 injector: the same XTEST surface the F1-F3
// captures used; compiled by capture_f4.sh.)
#include <X11/Xlib.h>
#include <X11/keysym.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

extern int XTestFakeMotionEvent(Display *, int, int, int, unsigned long);
extern int XTestFakeButtonEvent(Display *, unsigned int, Bool, unsigned long);
extern int XTestFakeKeyEvent(Display *, unsigned int, Bool, unsigned long);

int main(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "usage: %s DISPLAY move X Y | click X Y | wheel X Y up|down [ticks]\n", argv[0]);
        return 2;
    }
    Display *d = XOpenDisplay(argv[1]);
    if (!d) {
        fprintf(stderr, "inject: cannot open display %s\n", argv[1]);
        return 1;
    }
    int screen = DefaultScreen(d);
    const char *action = argv[2];
    if (strcmp(action, "move") == 0 && argc >= 5) {
        XTestFakeMotionEvent(d, screen, atoi(argv[3]), atoi(argv[4]), 0);
        XFlush(d);
        usleep(200000);
    } else if (strcmp(action, "click") == 0 && argc >= 5) {
        XTestFakeMotionEvent(d, screen, atoi(argv[3]), atoi(argv[4]), 0);
        XFlush(d);
        usleep(150000);
        XTestFakeButtonEvent(d, 1, True, 0);
        XFlush(d);
        usleep(80000);
        XTestFakeButtonEvent(d, 1, False, 0);
        XFlush(d);
        usleep(350000);
    } else if (strcmp(action, "wheel") == 0 && argc >= 6) {
        int x = atoi(argv[3]), y = atoi(argv[4]);
        int up = strcmp(argv[5], "up") == 0;
        int ticks = (argc >= 7) ? atoi(argv[6]) : 8;
        XTestFakeMotionEvent(d, screen, x, y, 0);
        XFlush(d);
        usleep(250000);
        for (int t = 0; t < ticks; t++) {
            XTestFakeButtonEvent(d, up ? 4 : 5, True, 0);
            XTestFakeButtonEvent(d, up ? 4 : 5, False, 0);
            XFlush(d);
            usleep(60000);
        }
        usleep(400000);
    } else {
        fprintf(stderr, "bad action %s\n", action);
        XCloseDisplay(d);
        return 2;
    }
    XCloseDisplay(d);
    return 0;
}
