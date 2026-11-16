package dev.handset;

import android.accessibilityservice.AccessibilityServiceInfo;
import android.app.UiAutomation;
import android.graphics.Point;
import android.graphics.Rect;
import android.os.HandlerThread;
import android.os.Looper;
import android.view.Display;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;

import java.io.BufferedReader;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.lang.reflect.Constructor;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;

/**
 * Describes what is on the screen, for Handset's `handset ctl ui`.
 *
 * Android's `uiautomator dump` does the same, but starts from nothing every time, which takes about three
 * seconds. This stays running and answers one request per line on standard input:
 *
 *   dump   -> "ok <n>\n" and n bytes of XML in uiautomator's format, or "err <message>\n"
 *   ping   -> "ok 0\n"
 *   quit   -> exits (so does the end of the input, which is how it goes when Handset or adb goes away)
 *
 * A phone does not need this running for long (it holds about a hundred megabytes), so it ends by itself after a
 * minute without a request; Handset starts it again when it is next wanted.
 *
 * The screen is read through a UiAutomation connection that is made for each request and dropped straight
 * after, and it asks Android not to switch off the person's own accessibility services (TalkBack and the
 * like) while it is connected.
 */
public final class Helper {
    /** UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES */
    private static final int DONT_SUPPRESS = 1;

    private static final long IDLE_MS = 60_000;

    private static HandlerThread thread;
    private static volatile long lastRequest = System.currentTimeMillis();

    public static void main(String[] args) throws Exception {
        // standard output carries the protocol and nothing else
        OutputStream out = new FileOutputStream(FileDescriptor.out);
        System.setOut(System.err);
        thread = new HandlerThread("handset-ui");
        thread.setDaemon(true);
        thread.start();

        Thread watchdog = new Thread(() -> {
            while (true) {
                try {
                    Thread.sleep(5_000);
                } catch (InterruptedException e) {
                    return;
                }
                if (System.currentTimeMillis() - lastRequest > IDLE_MS) {
                    System.exit(0);
                }
            }
        });
        watchdog.setDaemon(true);
        watchdog.start();

        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        for (String line; (line = in.readLine()) != null; ) {
            line = line.trim();
            lastRequest = System.currentTimeMillis();
            if (line.equals("quit")) {
                return;
            }
            byte[] body;
            String error = null;
            try {
                body = line.equals("dump") ? dump() : line.equals("ping") ? new byte[0] : null;
                if (body == null) {
                    error = "unknown request";
                }
            } catch (Throwable t) {
                body = null;
                error = t.getClass().getSimpleName() + ": " + t.getMessage();
            }
            if (body != null) {
                out.write(("ok " + body.length + "\n").getBytes(StandardCharsets.UTF_8));
                out.write(body);
            } else {
                out.write(("err " + error.replace('\n', ' ') + "\n").getBytes(StandardCharsets.UTF_8));
            }
            out.flush();
            lastRequest = System.currentTimeMillis();
        }
    }

    private static UiAutomation connect() throws Exception {
        Class<?> iface = Class.forName("android.app.IUiAutomationConnection");
        Object connection = Class.forName("android.app.UiAutomationConnection").getConstructor().newInstance();
        Constructor<UiAutomation> make = UiAutomation.class.getDeclaredConstructor(Looper.class, iface);
        UiAutomation automation = make.newInstance(thread.getLooper(), connection);
        automation.getClass().getMethod("connect", int.class).invoke(automation, DONT_SUPPRESS);
        return automation;
    }

    private static byte[] dump() throws Exception {
        UiAutomation automation = connect();
        try {
            // what `uiautomator dump --compressed` asks for: leave out views that say nothing to a screen reader
            AccessibilityServiceInfo info = automation.getServiceInfo();
            info.flags &= ~AccessibilityServiceInfo.FLAG_INCLUDE_NOT_IMPORTANT_VIEWS;
            info.flags |= AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS;
            automation.setServiceInfo(info);

            // the first moments after connecting have no window to show
            AccessibilityNodeInfo root = null;
            for (int i = 0; i < 80 && root == null; i++) {
                root = automation.getRootInActiveWindow();
                if (root == null) {
                    Thread.sleep(25);
                }
            }
            if (root == null) {
                throw new IllegalStateException("no window to describe");
            }
            Point size = displaySize();
            StringBuilder xml = new StringBuilder(16 * 1024);
            // the size is of the screen as it is held now, so the rotation is left at 0
            xml.append("Physical size: ").append(size.x).append('x').append(size.y).append('\n');
            xml.append("<?xml version='1.0' encoding='UTF-8' standalone='yes' ?><hierarchy rotation=\"0\">");
            // what can be seen of an element is where to tap it: not past the display, not past its window
            Rect visible = new Rect(0, 0, size.x, size.y);
            AccessibilityWindowInfo window = root.getWindow();
            if (window != null) {
                Rect w = new Rect();
                window.getBoundsInScreen(w);
                visible.intersect(w);
            }
            node(xml, root, 0, visible);
            xml.append("</hierarchy>");
            return xml.toString().getBytes(StandardCharsets.UTF_8);
        } finally {
            automation.getClass().getMethod("disconnect").invoke(automation);
        }
    }

    /** The real screen in pixels, in the way it is turned now. */
    private static Point displaySize() throws Exception {
        Class<?> global = Class.forName("android.hardware.display.DisplayManagerGlobal");
        Object instance = global.getMethod("getInstance").invoke(null);
        Method real = global.getMethod("getRealDisplay", int.class);
        Display display = (Display) real.invoke(instance, 0);
        Point p = new Point();
        display.getRealSize(p);
        return p;
    }

    private static void node(StringBuilder x, AccessibilityNodeInfo n, int index, Rect visible) {
        Rect b = new Rect();
        n.getBoundsInScreen(b);
        if (!b.intersect(visible)) {
            b.setEmpty();
        }
        x.append("<node index=\"").append(index).append('"');
        attr(x, "text", n.getText());
        attr(x, "resource-id", n.getViewIdResourceName());
        attr(x, "class", n.getClassName());
        attr(x, "package", n.getPackageName());
        attr(x, "content-desc", n.getContentDescription());
        flag(x, "checkable", n.isCheckable());
        flag(x, "checked", n.isChecked());
        flag(x, "clickable", n.isClickable());
        flag(x, "enabled", n.isEnabled());
        flag(x, "focusable", n.isFocusable());
        flag(x, "focused", n.isFocused());
        flag(x, "scrollable", n.isScrollable());
        flag(x, "long-clickable", n.isLongClickable());
        flag(x, "password", n.isPassword());
        flag(x, "selected", n.isSelected());
        x.append(" bounds=\"[").append(b.left).append(',').append(b.top).append("][").append(b.right).append(',')
                .append(b.bottom).append("]\"");
        int count = n.getChildCount();
        boolean open = false;
        int kept = 0;
        for (int i = 0; i < count; i++) {
            AccessibilityNodeInfo child = n.getChild(i);
            if (child == null || !child.isVisibleToUser()) {
                continue;
            }
            if (!open) {
                x.append('>');
                open = true;
            }
            node(x, child, kept++, visible);
        }
        x.append(open ? "</node>" : " />");
    }

    private static void flag(StringBuilder x, String name, boolean on) {
        x.append(' ').append(name).append("=\"").append(on).append('"');
    }

    private static void attr(StringBuilder x, String name, CharSequence value) {
        x.append(' ').append(name).append("=\"");
        if (value != null) {
            for (int i = 0; i < value.length(); i++) {
                char c = value.charAt(i);
                switch (c) {
                    case '&': x.append("&amp;"); break;
                    case '<': x.append("&lt;"); break;
                    case '>': x.append("&gt;"); break;
                    case '"': x.append("&quot;"); break;
                    default:
                        // what XML 1.0 cannot hold, and the line breaks a person typed
                        if (c < 0x20 || c == 0x7f) {
                            x.append("&#").append((int) c).append(';');
                        } else {
                            x.append(c);
                        }
                }
            }
        }
        x.append('"');
    }
}
