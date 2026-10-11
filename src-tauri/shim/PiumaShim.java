import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;

public final class PiumaShim {
    private static final String PLACEHOLDER = "@@PIUMA_TOKEN@@";

    public static void main(String[] args) throws Throwable {
        if (args.length < 1) {
            System.err.println("PiumaShim: missing main class");
            System.exit(2);
        }
        ByteArrayOutputStream buf = new ByteArrayOutputStream();
        for (int b = System.in.read(); b != -1 && b != '\n'; b = System.in.read()) {
            buf.write(b);
        }
        String token = new String(buf.toByteArray(), StandardCharsets.UTF_8).trim();

        String[] rest = new String[args.length - 1];
        for (int i = 1; i < args.length; i++) {
            rest[i - 1] = args[i].replace(PLACEHOLDER, token);
        }
        try {
            Method main = Class.forName(args[0]).getMethod("main", String[].class);
            main.invoke(null, (Object) rest);
        } catch (InvocationTargetException e) {
            throw e.getCause() != null ? e.getCause() : e;
        }
    }
}
