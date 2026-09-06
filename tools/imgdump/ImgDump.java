// Dump the geometry of a compiled Garmin IMG using mkgmap's own reader.
//
// This is the ground truth for the render harness (SPEC.md FR-CART7): it reports
// exactly what a device would find in the map -- object types, zoom levels and
// coordinates -- rather than what we believe the style should have produced.
//
// Usage: java -cp mkgmap.jar:. ImgDump <file.img> [level]
// Output: TSV on stdout.
//   BOUNDS  minLat maxLat minLon maxLon
//   LEVEL   level resolution
//   S|L|P   level  type  label  lat,lon lat,lon ...

import uk.me.parabola.imgfmt.app.Area;
import uk.me.parabola.imgfmt.app.Coord;
import uk.me.parabola.imgfmt.app.map.MapReader;
import uk.me.parabola.imgfmt.app.trergn.Point;
import uk.me.parabola.imgfmt.app.trergn.Polygon;
import uk.me.parabola.imgfmt.app.trergn.Polyline;
import uk.me.parabola.imgfmt.app.trergn.Zoom;

import java.io.BufferedWriter;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.List;

public class ImgDump {

    /** Garmin stores coordinates as 24-bit units of 360/2^24 degrees. */
    private static double deg(int v) {
        return v * 360.0 / (1 << 24);
    }

    private static String label(Object o) {
        try {
            java.lang.reflect.Method m = o.getClass().getMethod("getLabel");
            Object l = m.invoke(o);
            if (l == null) return "";
            Object t = l.getClass().getMethod("getText").invoke(l);
            return t == null ? "" : t.toString().replace('\t', ' ');
        } catch (Exception e) {
            return "";
        }
    }

    private static void coords(PrintWriter out, List<Coord> pts) {
        for (Coord c : pts) {
            out.print('\t');
            out.print(String.format("%.6f,%.6f", c.getLatDegrees(), c.getLonDegrees()));
        }
        out.println();
    }

    public static void main(String[] args) throws Exception {
        if (args.length < 1) {
            System.err.println("usage: ImgDump <file.img> [level]");
            System.exit(2);
        }
        String path = args[0];
        Integer only = args.length > 1 ? Integer.parseInt(args[1]) : null;

        PrintWriter out = new PrintWriter(new BufferedWriter(
                new OutputStreamWriter(System.out, StandardCharsets.UTF_8)));

        try (MapReader mr = new MapReader(path)) {
            Area b = mr.getTreBounds();
            out.printf("BOUNDS\t%.6f\t%.6f\t%.6f\t%.6f%n",
                    deg(b.getMinLat()), deg(b.getMaxLat()),
                    deg(b.getMinLong()), deg(b.getMaxLong()));

            for (Zoom z : mr.getLevels()) {
                out.printf("LEVEL\t%d\t%d%n", z.getLevel(), z.getResolution());
            }

            for (Zoom z : mr.getLevels()) {
                int lv = z.getLevel();
                if (only != null && lv != only) continue;

                for (Polygon p : mr.shapesForLevel(lv, MapReader.WITH_EXT_TYPE_DATA)) {
                    out.printf("S\t%d\t0x%x\t%s", lv, p.getType(), label(p));
                    coords(out, p.getPoints());
                }
                for (Polyline p : mr.linesForLevel(lv)) {
                    out.printf("L\t%d\t0x%x\t%s", lv, p.getType(), label(p));
                    coords(out, p.getPoints());
                }
                for (Point p : mr.pointsForLevel(lv, MapReader.WITH_EXT_TYPE_DATA)) {
                    Coord loc = p.getLocation();
                    out.printf("P\t%d\t0x%x\t%s\t%.6f,%.6f%n", lv, p.getType(), label(p),
                            loc.getLatDegrees(), loc.getLonDegrees());
                }
            }
        }
        out.flush();
    }
}
