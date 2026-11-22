/* ui_b3_inject.c - UI-B3 proof injector: move the pointer to (x, y) and
 * primary-click there, so the real Isolith binary under Xvfb picks the datum
 * plane at the viewport centre and enters sketch mode. Same XTEST technique
 * as proof/ui-a8/xtest_inject.c (which adds AND/PAL/ORB modes); this one only
 * needs one clean click (press + release, no motion between). */
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
#include <unistd.h>

extern int XTestFakeButtonEvent(Display *d, unsigned int button, Bool is_press,
                                unsigned long delay);
extern int XTestFakeMotionEvent(Display *d, int screen, int x, int y,
                                unsigned long delay);

static Window find_isolith(Display *d, Window root) {
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
    for (unsigned int i = 0; i < n && !found; i++) found = find_isolith(d, kids[i]);
    if (kids) XFree(kids);
    return found;
}

int main(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "usage: %s <display> <x> <y>\n", argv[0]);
        return 2;
    }
    Display *d = XOpenDisplay(argv[1]);
    if (!d) { fprintf(stderr, "cannot open display %s\n", argv[1]); return 1; }
    int screen = DefaultScreen(d);
    Window root = DefaultRootWindow(d);
    Window app = 0;
    for (int i = 0; i < 120 && !app; i++) {
        app = find_isolith(d, root);
        if (!app) usleep(250000);
    }
    if (!app) { fprintf(stderr, "window not found\n"); XCloseDisplay(d); return 1; }
    XFlush(d);
    usleep(500000);
    XSetInputFocus(d, app, RevertToPointerRoot, CurrentTime);
    XFlush(d);
    usleep(200000);

    int x = atoi(argv[2]);
    int y = atoi(argv[3]);
    XTestFakeMotionEvent(d, screen, x, y, 0);
    XFlush(d); usleep(200000);
    XTestFakeButtonEvent(d, 1, True, 0);
    XFlush(d); usleep(200000);
    XTestFakeButtonEvent(d, 1, False, 0);
    XFlush(d); usleep(400000);
    XCloseDisplay(d);
    return 0;
}
