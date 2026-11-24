/* ui_c7_inject.c - wave-C close (UI-C7) proof injector: drive the real Isolith
 * binary under Xvfb through XTEST. Same technique as the C1/C2/C5 injectors;
 * supports click / move / key so a bash capture can script the tree frame, the
 * property-panel selection and the measure surface in one gesture sequence.
 *
 * usage: ui_c7_inject <display> click <x> <y>
 *        ui_c7_inject <display> move  <x> <y>
 *        ui_c7_inject <display> key   <keysym>     (e.g. "m")
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

static void focus_app(Display *d, Window app) {
    XSetInputFocus(d, app, RevertToPointerRoot, CurrentTime);
    XFlush(d);
    usleep(200000);
}

int main(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "usage: %s <display> <click|move|key> ...\n", argv[0]);
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
        XFlush(d); usleep(400000);
    } else if (strcmp(action, "key") == 0 && argc >= 4) {
        KeySym sym = XStringToKeysym(argv[3]);
        if (sym == NoSymbol) {
            fprintf(stderr, "unknown keysym %s\n", argv[3]);
            XCloseDisplay(d);
            return 1;
        }
        KeyCode code = XKeysymToKeycode(d, sym);
        XTestFakeKeyEvent(d, code, True, 0);
        XFlush(d); usleep(200000);
        XTestFakeKeyEvent(d, code, False, 0);
        XFlush(d); usleep(400000);
    } else {
        fprintf(stderr, "bad action %s\n", action);
        XCloseDisplay(d);
        return 2;
    }
    XCloseDisplay(d);
    return 0;
}
