/* ui_d6_inject.c - UI-D6 wave-D close proof injector: drive the real Isolith binary under
 * Xvfb through XTEST so the capture can show the GPU pick surface.
 *
 * usage: ui_d6_inject <display> move   <x> <y>
 *        ui_d6_inject <display> click  <x> <y>
 *        ui_d6_inject <display> drag   <x0> <y0> <x1> <y1> [alt|none]
 *        ui_d6_inject <display> dstart <x0> <y0> alt      (hold button + Alt)
 *        ui_d6_inject <display> dto    <x1> <y1>          (move while held)
 *        ui_d6_inject <display> dend                      (release button+Alt)
 *
 * `drag ... alt` holds Alt_L, presses button 1 at (x0,y0), steps through the
 * midpoint and (x1,y1) (so egui sees a real multi-frame drag), releases the
 * button and Alt. The `dstart`/`dto`/`dend` trio lets a capture grab the
 * frame WHILE the drag is held, showing the box-select rubber band live.
 * This reproduces the UI-D6 Alt+left-drag box-select + nav click/move in the running
 * app. Each invocation is one gesture, like the wave-C injectors.
 */
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/keysym.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>

extern int XTestFakeButtonEvent(Display *d, unsigned int button, Bool is_press,
                                unsigned long delay);
extern int XTestFakeMotionEvent(Display *d, int screen, int x, int y,
                                unsigned long delay);
extern int XTestFakeKeyEvent(Display *d, unsigned int keycode, Bool is_press,
                             unsigned long delay);

static Window find_isol(Display *d, Window root) {
    static Window found = 0;
    Window r, parent;
    Window *kids = NULL;
    unsigned int n = 0;
    char *name = NULL;
    if (found) return found;
    if (XFetchName(d, root, &name) && name) {
        if (strcasestr(name, "isol") || strcasestr(name, "modeler")) {
            found = root; XFree(name); return found;
        }
        XFree(name);
    }
    if (!XQueryTree(d, root, &r, &parent, &kids, &n)) return 0;
    for (unsigned int i = 0; i < n && !found; i++) found = find_isol(d, kids[i]);
    if (kids) XFree(kids);
    return found;
}

static void focus_app(Display *d, Window app) {
    XSetInputFocus(d, app, RevertToPointerRoot, CurrentTime);
    XFlush(d);
    usleep(200000);
}

int main(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "usage: %s <display> <move|click|drag|dstart|dto|dend> ...\n", argv[0]);
        return 2;
    }
    Display *d = XOpenDisplay(argv[1]);
    if (!d) { fprintf(stderr, "cannot open display %s\n", argv[1]); return 1; }
    int screen = DefaultScreen(d);
    Window root = DefaultRootWindow(d);
    Window app = 0;
    for (int i = 0; i < 120 && !app; i++) {
        app = find_isol(d, root);
        if (!app) usleep(250000);
    }
    if (!app) { fprintf(stderr, "window not found\n"); XCloseDisplay(d); return 1; }
    XFlush(d);
    usleep(500000);
    focus_app(d, app);

    const char *action = argv[2];
    if (strcmp(action, "click") == 0 && argc >= 5) {
        int x = atoi(argv[3]);
        int y = atoi(argv[4]);
        XTestFakeMotionEvent(d, screen, x, y, 0);
        XFlush(d); usleep(200000);
        XTestFakeButtonEvent(d, 1, True, 0);
        XFlush(d); usleep(200000);
        XTestFakeButtonEvent(d, 1, False, 0);
        XFlush(d); usleep(400000);
    } else if (strcmp(action, "move") == 0 && argc >= 5) {
        int x = atoi(argv[3]);
        int y = atoi(argv[4]);
        XTestFakeMotionEvent(d, screen, x, y, 0);
        XFlush(d); usleep(500000);
    } else if (strcmp(action, "drag") == 0 && argc >= 7) {
        int x0 = atoi(argv[3]), y0 = atoi(argv[4]);
        int x1 = atoi(argv[5]), y1 = atoi(argv[6]);
        int alt = (argc >= 8 && strcmp(argv[7], "alt") == 0);
        XTestFakeMotionEvent(d, screen, x0, y0, 0);
        XFlush(d); usleep(200000);
        if (alt) {
            XTestFakeKeyEvent(d, XKeysymToKeycode(d, XK_Alt_L), True, 0);
            XFlush(d); usleep(150000);
        }
        XTestFakeButtonEvent(d, 1, True, 0);
        XFlush(d); usleep(200000);
        XTestFakeMotionEvent(d, screen, (x0 + x1) / 2, (y0 + y1) / 2, 0);
        XFlush(d); usleep(200000);
        XTestFakeMotionEvent(d, screen, x1, y1, 0);
        XFlush(d); usleep(200000);
        XTestFakeButtonEvent(d, 1, False, 0);
        XFlush(d); usleep(300000);
        if (alt) {
            XTestFakeKeyEvent(d, XKeysymToKeycode(d, XK_Alt_L), False, 0);
            XFlush(d); usleep(150000);
        }
    } else if (strcmp(action, "dstart") == 0 && argc >= 6) {
        int x = atoi(argv[3]), y = atoi(argv[4]);
        XTestFakeMotionEvent(d, screen, x, y, 0);
        XFlush(d); usleep(200000);
        XTestFakeKeyEvent(d, XKeysymToKeycode(d, XK_Alt_L), True, 0);
        XFlush(d); usleep(150000);
        XTestFakeButtonEvent(d, 1, True, 0);
        XFlush(d); usleep(300000);
    } else if (strcmp(action, "dto") == 0 && argc >= 5) {
        int x = atoi(argv[3]), y = atoi(argv[4]);
        XTestFakeMotionEvent(d, screen, x, y, 0);
        XFlush(d); usleep(300000);
    } else if (strcmp(action, "dend") == 0 && argc >= 3) {
        XTestFakeButtonEvent(d, 1, False, 0);
        XFlush(d); usleep(300000);
        XTestFakeKeyEvent(d, XKeysymToKeycode(d, XK_Alt_L), False, 0);
        XFlush(d); usleep(200000);
    } else if (strcmp(action, "wheel") == 0 && argc >= 5) {
        /* wheel <x> <y> <up|down> [ticks]  -  egui ScrollArea reads button
         * 4/5 as wheel deltas. Park the pointer over the mate tool, then
         * emit `ticks` (default 8) wheel notches so content under the
         * fixed-height assembly region scrolls into view in the real binary. */
        int x = atoi(argv[3]), y = atoi(argv[4]);
        int up = (argc >= 6 && strcmp(argv[5], "up") == 0);
        int ticks = (argc >= 7) ? atoi(argv[6]) : 8;
        XTestFakeMotionEvent(d, screen, x, y, 0);
        XFlush(d); usleep(250000);
        for (int t = 0; t < ticks; t++) {
            XTestFakeButtonEvent(d, up ? 4 : 5, True, 0);
            XTestFakeButtonEvent(d, up ? 4 : 5, False, 0);
            XFlush(d); usleep(60000);
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

