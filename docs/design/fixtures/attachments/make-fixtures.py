#!/usr/bin/env python3
"""Writes the attachment sample files the fixture core serves (lane attachments-mac).

Every file is drawn here, never copied from anyone's folders: a 2-page PDF, a PNG, three JPEG photos, a
3-second MP4, a DOCX, a ZIP and an SVG. The PDF, PNG, DOCX, ZIP and SVG come from the standard library;
the JPEGs are the PNG drawings converted by `sips` (macOS); the MP4 is drawn by `ffmpeg` (Homebrew).

Run: python3 docs/design/fixtures/attachments/make-fixtures.py
Prints one line per file and exits 1 when a file could not be written.
"""
import io, math, os, struct, subprocess, sys, zipfile, zlib

STAMP = (2026, 11, 8, 12, 0, 0)                      # every zip entry carries this date, so a rerun writes the same bytes


def put(z, name, data):
    z.writestr(zipfile.ZipInfo(name, date_time=STAMP), data, compress_type=zipfile.ZIP_DEFLATED)

HERE = os.path.dirname(os.path.abspath(__file__))


def png_bytes(w, h, pixel):
    raw = bytearray()
    for y in range(h):
        raw.append(0)
        for x in range(w):
            raw.extend(pixel(x, y))
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b""))


def mix(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * max(0.0, min(1.0, t))) for i in range(3))


def scene(top, bottom, sun, hills):
    """A small landscape: a sky gradient, a soft sun, two rolling hills."""
    def pixel(x, y, w=480, h=320):
        c = mix(top, bottom, y / h)
        d = math.hypot(x - sun[0], y - sun[1])
        if d < 120:
            c = mix(c, (255, 244, 214), (1 - d / 120) ** 2)
        for i, (base, amp, col) in enumerate(hills):
            if y > base + amp * math.sin(x / (60 + 30 * i) + i):
                c = mix(col, tuple(v // 2 for v in col), (y - base) / (h - base + 1))
        return c
    return pixel


SCENES = {
    "photo-dawn": scene((92, 130, 220), (250, 196, 160), (330, 150), [(210, 18, (70, 110, 90)), (250, 12, (40, 80, 70))]),
    "photo-sea": scene((60, 120, 200), (170, 215, 240), (120, 110), [(230, 6, (30, 90, 150)), (275, 4, (20, 60, 110))]),
    "photo-dusk": scene((40, 30, 90), (230, 120, 110), (240, 200), [(220, 22, (60, 40, 80)), (262, 14, (30, 20, 50))]),
}


def pdf_bytes():
    """Two A5 pages: a quote with a coloured header band and a table; page 2 the terms."""
    pages = [
        ["0.16 0.38 0.92 rg 0 520 420 75 re f", "1 1 1 rg BT /F2 22 Tf 32 552 Td (Quotation) Tj ET",
         "1 1 1 rg BT /F1 11 Tf 32 532 Td (Q1 print run, Linea Supply) Tj ET",
         "0.2 0.2 0.22 rg BT /F2 12 Tf 32 480 Td (Item) Tj 220 0 Td (Units) Tj 90 0 Td (EUR) Tj ET",
         "0.85 0.87 0.92 RG 1 w 32 470 m 388 470 l S",
         "0.2 0.2 0.22 rg BT /F1 11 Tf 32 450 Td (Brochure A5, 16 pages) Tj 220 0 Td (12,000) Tj 90 0 Td (10,080) Tj ET",
         "BT /F1 11 Tf 32 428 Td (Delivery to the warehouse) Tj 220 0 Td (1) Tj 90 0 Td (420) Tj ET",
         "0.95 0.96 0.99 rg 32 360 356 40 re f",
         "0.16 0.38 0.92 rg BT /F2 14 Tf 44 375 Td (Total 10,500 EUR) Tj ET",
         "0.45 0.47 0.52 rg BT /F1 9 Tf 32 40 Td (Page 1 of 2) Tj ET"],
        ["0.2 0.2 0.22 rg BT /F2 16 Tf 32 552 Td (Terms) Tj ET",
         "BT /F1 11 Tf 32 520 Td (Valid until 30 June.) Tj 0 -18 Td (Payment within 30 days of delivery.) Tj"
         " 0 -18 Td (Proof approved before the run starts.) Tj ET",
         "0.16 0.38 0.92 RG 2 w 32 430 m 200 430 l S",
         "0.45 0.47 0.52 rg BT /F1 9 Tf 32 40 Td (Page 2 of 2) Tj ET"],
    ]
    objs = ["<< /Type /Catalog /Pages 2 0 R >>", None,
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>"]
    kids = []
    for p in pages:
        stream = "\n".join(p).encode()
        objs.append("<< /Length %d >>\nstream\n%s\nendstream" % (len(stream), stream.decode()))
        objs.append("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 420 595] /Contents %d 0 R "
                    "/Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> >>" % len(objs))
        kids.append(len(objs))
    objs[1] = "<< /Type /Pages /Kids [%s] /Count %d >>" % (" ".join("%d 0 R" % k for k in kids), len(kids))
    out, offsets = io.BytesIO(), []
    out.write(b"%PDF-1.4\n")
    for i, o in enumerate(objs, 1):
        offsets.append(out.tell())
        out.write(("%d 0 obj\n%s\nendobj\n" % (i, o)).encode())
    x = out.tell()
    out.write(("xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)).encode())
    for off in offsets:
        out.write(("%010d 00000 n \n" % off).encode())
    out.write(("trailer << /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, x)).encode())
    return out.getvalue()


def docx_bytes():
    b = io.BytesIO()
    with zipfile.ZipFile(b, "w", zipfile.ZIP_DEFLATED) as z:
        put(z, "[Content_Types].xml", '<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
                   '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>'
                   '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>')
        put(z, "_rels/.rels", '<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
                   '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>')
        paras = ["Agenda, 19 November", "1. Welcome and goals", "2. Brand book v3 review", "3. Print run timing", "4. Next steps"]
        body = "".join('<w:p><w:r>%s<w:t>%s</w:t></w:r></w:p>' % ('<w:rPr><w:b/><w:sz w:val="36"/></w:rPr>' if i == 0 else "", t) for i, t in enumerate(paras))
        put(z, "word/document.xml", '<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
                   '<w:body>%s</w:body></w:document>' % body)
    return b.getvalue()


def zip_bytes(pdf, png):
    b = io.BytesIO()
    with zipfile.ZipFile(b, "w", zipfile.ZIP_DEFLATED) as z:
        put(z, "press-kit/quote.pdf", pdf)
        put(z, "press-kit/cover.png", png)
        put(z, "press-kit/README.txt", "Press kit, fixture sample.\n")
    return b.getvalue()


SVG = ('<svg xmlns="http://www.w3.org/2000/svg" width="480" height="160" viewBox="0 0 480 160">'
       '<rect width="480" height="160" rx="18" fill="#F4F6FB"/><circle cx="80" cy="80" r="44" fill="#2961EB"/>'
       '<path d="M58 70l22 16 22-16" stroke="#fff" stroke-width="7" fill="none" stroke-linecap="round" stroke-linejoin="round"/>'
       '<text x="146" y="96" font-family="Helvetica" font-weight="700" font-size="46" fill="#1C1E24">wordmark</text></svg>')


def corpus(sketch_png):
    """gallery-corpus.json (whoosh-search-corpus/1, loaded with `fixture corpus`): the gallery's three cases."""
    import base64, json
    photos = ["IMG_%d.jpg" % n for n in range(3101, 3111)] + ["screenshot-%d.png" % n for n in (1, 2)]
    videos = ["walkthrough-%d.mp4" % n for n in (1, 2)]
    docs = (["spec-part-%d.pdf" % n for n in range(1, 9)] + ["minutes-%d.docx" % n for n in range(1, 6)]
            + ["assets-%d.zip" % n for n in range(1, 4)])
    def row(i, subject, body, attachments, sender="Sophie Chammah", address="sophie@family-example.net", account="acc-10"):
        return {"id": "msg-g%d" % i, "thread": "thr-g%d" % i, "account": account, "from_name": sender, "from": address,
                "date": "2026-11-0%dT10:00:00" % (8 - i), "subject": subject, "body": body, "attachments": attachments}
    return {
        "schema": "whoosh-search-corpus/1",
        "note": "attachments-mac gallery fixture: three photos; thirty files (12 images, 2 videos, 16 documents); inline images",
        "messages": [
            row(1, "Photos from the garden", "Here are the ones from the garden. The last one is the keeper.",
                ["IMG_2041.jpg", "IMG_2044.jpg", "IMG_2051.jpg"]),
            row(2, "Site visit: everything from today", "All the photos, the two walkthrough videos and the documents from the site visit.",
                photos[:6] + videos[:1] + docs[:8] + photos[6:] + videos[1:] + docs[8:],
                sender="Karim Haddad", address="karim@haddad-example.com", account="acc-04"),
            row(3, "The floor plan, inline", "Here is the plan as we discussed [[inline:floor-plan.png]] and the sketch from this morning "
                "[[inline-data:%s]] with the notes attached." % base64.b64encode(sketch_png).decode(),
                ["floor-plan.png", "notes.pdf"], sender="Marie Lef\u00e8vre", address="marie@lefevre-example.fr", account="acc-02"),
        ],
    }


def main():
    failed = 0
    def write(name, data):
        with open(os.path.join(HERE, name), "wb") as f:
            f.write(data)
        print("wrote %s (%d bytes)" % (name, len(data)))

    pdf = pdf_bytes()
    write("sample-2-pages.pdf", pdf)
    cover = png_bytes(480, 320, SCENES["photo-sea"])
    write("sample.png", cover)
    for name, px in SCENES.items():
        tmp = os.path.join(HERE, name + ".tmp.png")
        with open(tmp, "wb") as f:
            f.write(png_bytes(480, 320, px))
        r = subprocess.run(["sips", "-s", "format", "jpeg", "-s", "formatOptions", "70", tmp, "--out", os.path.join(HERE, name + ".jpg")],
                           capture_output=True, text=True)
        os.remove(tmp)
        if r.returncode != 0:
            failed += 1
            print("FAILED %s.jpg: %s" % (name, r.stderr.strip()))
        else:
            print("wrote %s.jpg" % name)
    write("sample.docx", docx_bytes())
    write("sample.zip", zip_bytes(pdf, cover))
    write("sample.svg", SVG.encode())
    import json
    sketch = png_bytes(96, 64, lambda x, y: SCENES["photo-dusk"](x * 5, y * 5))
    write("gallery-corpus.json", (json.dumps(corpus(sketch), indent=1, ensure_ascii=False) + "\n").encode())
    mp4 = os.path.join(HERE, "sample-3-seconds.mp4")
    if os.path.exists(mp4):                                    # the encoder is not byte-stable: keep the committed clip
        print("kept sample-3-seconds.mp4 (%d bytes)" % os.path.getsize(mp4))
        print("%d files failed" % failed)
        return 1 if failed else 0
    r = subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-f", "lavfi", "-i",
                        "gradients=s=320x180:d=3:speed=0.02:c0=0x2961EB:c1=0xF2B8A0:c2=0x46B49A,format=yuv420p",
                        "-c:v", "libx264", "-preset", "veryslow", "-crf", "30", "-threads", "1", "-map_metadata", "-1",
                        "-fflags", "+bitexact", "-flags:v", "+bitexact", "-movflags", "+faststart", mp4],
                       capture_output=True, text=True)
    if r.returncode != 0:
        failed += 1
        print("FAILED sample-3-seconds.mp4: %s" % r.stderr.strip())
    else:
        print("wrote sample-3-seconds.mp4 (%d bytes)" % os.path.getsize(mp4))
    print("%d files failed" % failed)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
