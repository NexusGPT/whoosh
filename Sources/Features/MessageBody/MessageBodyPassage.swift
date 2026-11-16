import Foundation
import SwiftUI

/// The passage that made a mail a search result, marked inside the mail (docs/SPEC.md A152, P1; the drawing is
/// docs/design/boards/search-r3-parts/designs.js: `.pm`, a tint of the accent colour at 26 percent behind the passage, radius 3, the text colour unchanged).
///
/// The rule, one place for the reader's plain text and for the page: the mail's text and the passage are compared with white space folded to one space
/// (case and accents do not matter); the passage may be cut at its ends with "…" (or "..."), only the part between the dots is looked for; the first
/// occurrence is the passage. With typed letters (a match), each occurrence of the letters INSIDE the passage is the marked part (the ranges rule of
/// `SearchLetters`, the same one the list's bold uses); with none (a related mail), the whole passage is. Pure: text in, ranges out; nothing is read from the
/// network and nothing is computed while a frame is drawn (the caller keeps the answer, `PassageMemo`).
enum PassageMark {
    /// The tint's share of the accent colour (the drawing's `color-mix(in srgb, var(--accent) 26%, transparent)`).
    static let tint = 0.26
    /// The drawing's corner radius, in points.
    static let radius = 3

    /// The text on one line, and where each of its characters stands in the source text.
    struct Folded {
        let line: String
        let origin: [String.Index]
    }

    /// White space runs (line breaks too) become one space; leading and trailing white space goes. The same fold as `SearchLetters.oneLine`, with the map back.
    static func fold(_ text: String) -> Folded {
        var line = "", origin: [String.Index] = []
        var pending: String.Index?
        for i in text.indices {
            let c = text[i]
            if c.isWhitespace { if pending == nil { pending = i }; continue }
            if let p = pending, !line.isEmpty { line.append(" "); origin.append(p) }
            pending = nil
            line.append(c); origin.append(i)
        }
        return Folded(line: line, origin: origin)
    }

    /// What is looked for: the passage on one line, without the dots at its ends ("… as agreed, we lowered …" -> "as agreed, we lowered").
    static func core(_ passage: String) -> String {
        var s = SearchLetters.oneLine(passage)
        var again = true
        while again {
            again = false
            for dots in ["…", "..."] {
                if s.hasPrefix(dots) { s = String(s.dropFirst(dots.count)); again = true }
                if s.hasSuffix(dots) { s = String(s.dropLast(dots.count)); again = true }
            }
            s = s.trimmingCharacters(in: .whitespaces)
        }
        return s
    }

    /// The passage and its marked parts as character offsets in the folded line; nil when the line does not hold the passage.
    struct Found: Equatable {
        let passage: Range<Int>
        let marks: [Range<Int>]
    }

    static func find(passage: String, letters: [String], in line: String) -> Found? {
        let needle = core(passage)
        guard !needle.isEmpty, let r = line.range(of: needle, options: [.caseInsensitive, .diacriticInsensitive]) else { return nil }
        let lo = line.distance(from: line.startIndex, to: r.lowerBound), hi = line.distance(from: line.startIndex, to: r.upperBound)
        let inside = String(line[r])
        var marks: [Range<Int>] = []
        for m in SearchLetters.ranges(letters, in: inside) {
            marks.append((lo + inside.distance(from: inside.startIndex, to: m.lowerBound))..<(lo + inside.distance(from: inside.startIndex, to: m.upperBound)))
        }
        return Found(passage: lo..<hi, marks: marks.isEmpty ? [lo..<hi] : marks)   // letters not found inside the passage: the passage as a whole
    }

    /// The marked ranges in `text` itself (its own white space and line breaks), for a text the reader draws as it is. [] when `text` does not hold the passage.
    static func ranges(passage: String, letters: [String], in text: String) -> [Range<String.Index>] {
        let f = fold(text)
        guard let found = find(passage: passage, letters: letters, in: f.line) else { return [] }
        return found.marks.compactMap { r in
            guard r.lowerBound < f.origin.count, r.upperBound <= f.origin.count, r.lowerBound < r.upperBound else { return nil }
            return f.origin[r.lowerBound]..<text.index(after: f.origin[r.upperBound - 1])
        }
    }

    /// The marked parts as text (what a check reads without a picture).
    static func pieces(passage: String, letters: [String], in text: String) -> [String] {
        ranges(passage: passage, letters: letters, in: text).map { String(text[$0]) }
    }

    /// Where the passage starts in the text, 0 (the top) to 1 (the end): how far down a long mail the reader has to go.
    static func fraction(passage: String, in text: String) -> Double {
        let f = fold(text)
        guard !f.line.isEmpty, let found = find(passage: passage, letters: [], in: f.line) else { return 0 }
        return Double(found.passage.lowerBound) / Double(f.line.count)
    }

    // MARK: the web page

    /// What the page needs: the passage as the folded text holds it, and the marked spans as UTF-16 offsets from its start (a JavaScript string's own unit).
    struct Page: Equatable, Sendable {
        let text: String
        let spans: [[Int]]
    }

    static func page(passage: String, letters: [String], in text: String) -> Page? {
        let f = fold(text)
        guard let found = find(passage: passage, letters: letters, in: f.line) else { return nil }
        let chars = Array(f.line)
        let shown = String(chars[found.passage])
        let spans = found.marks.map { m -> [Int] in
            [String(chars[found.passage.lowerBound..<m.lowerBound]).utf16.count, String(chars[found.passage.lowerBound..<m.upperBound]).utf16.count]
        }
        return Page(text: shown, spans: spans)
    }

    /// The app's own script (page script stays off; the app's runs through the web view's evaluateJavaScript, as `WideMailFit.script` does) that marks the
    /// passage in the page: the page's text is folded the same way, the passage is found in it, and every marked span is wrapped in `<mark class="pm">`.
    /// Earlier marks go first. Nothing is loaded, only the page's own text nodes are split. Answers [marks made, where the first stands as a share of the
    /// page's height]. `nil` takes the marks off.
    static func script(_ page: Page?) -> String {
        let arg: String = {
            guard let page, let d = try? JSONEncoder().encode(Wire(t: page.text, s: page.spans)), let s = String(data: d, encoding: .utf8) else { return "null" }
            return s
        }()
        return """
        (function(a){
        var old=document.querySelectorAll('mark.pm');
        for(var i=0;i<old.length;i++){var m=old[i],p=m.parentNode;while(m.firstChild)p.insertBefore(m.firstChild,m);p.removeChild(m);p.normalize();}
        if(!a)return [0,0];
        var T=a.t,S=a.s,root=document.getElementById('m')||document.body,made=0;
        var roots=root.querySelectorAll('.ad,.od');if(!roots.length)roots=[root];
        function blk(x,R){var e=x.parentElement;while(e&&e!==R&&getComputedStyle(e).display.indexOf('inline')===0)e=e.parentElement;return e;}
        for(var r=0;r<roots.length;r++){
         var R=roots[r],w=document.createTreeWalker(R,NodeFilter.SHOW_TEXT),f='',map=[],sp=true,last=null,nd;
         while((nd=w.nextNode())){
          var b=blk(nd,R);
          if(last!==null&&b!==last&&!sp){f+=' ';map.push(null);sp=true;}
          last=b;
          var s=nd.nodeValue;
          for(var k=0;k<s.length;k++){var c=s.charAt(k);
           if(/\\s/.test(c)){if(!sp){f+=' ';map.push([nd,k]);sp=true;}}
           else{f+=c;map.push([nd,k]);sp=false;}}
         }
         var at=f.indexOf(T);if(at<0)at=f.toLowerCase().indexOf(T.toLowerCase());
         if(at<0)continue;
         var pieces=[],cur=null;
         for(var j=0;j<S.length;j++){
          for(var k=at+S[j][0];k<at+S[j][1];k++){var e=map[k];if(!e)continue;
           if(cur&&cur.nd===e[0]&&cur.e===e[1]){cur.e=e[1]+1;}
           else{if(cur)pieces.push(cur);cur={nd:e[0],s:e[1],e:e[1]+1};}}
          if(cur){pieces.push(cur);cur=null;}
         }
         for(var q=pieces.length-1;q>=0;q--){var P=pieces[q],mid=P.nd.splitText(P.s);mid.splitText(P.e-P.s);
          var mk=document.createElement('mark');mk.className='pm';mid.parentNode.replaceChild(mk,mid);mk.appendChild(mid);made++;}
        }
        var all=document.querySelectorAll('mark.pm'),h=document.body.getBoundingClientRect().height||1,y=0;
        for(var z=0;z<all.length;z++){var rc=all[z].getBoundingClientRect();if(rc.height>0){y=rc.top/h;break;}}
        return [made,y];
        })(\(arg))
        """
    }

    private struct Wire: Encodable { let t: String; let s: [[Int]] }

    /// The page's style for the mark (inside the document's own `<style>`): the accent colour at `tint`, radius, the text colour as it was. It outranks the
    /// app colours' `html.app body *{background:transparent!important}` by one class.
    static func css(accent: RGBA) -> String {
        func c(_ v: Double) -> Int { Int((v * 255).rounded()) }
        return "html.app mark.pm,html.original mark.pm{background:rgba(\(c(accent.red)),\(c(accent.green)),\(c(accent.blue)),\(tint))!important;border-radius:\(radius)px;padding:1px 2px;color:inherit!important}"
    }
}

/// The last answer, kept: a redraw of the pane asks again with the same mail, passage and letters and costs a comparison, never a scan of the text.
@MainActor
final class PassageMemo {
    static let shared = PassageMemo()
    private var key = "", page: PassageMark.Page?
    private var rangesKey = "", held: [Range<String.Index>] = []

    /// How many times the text was scanned (a test reads it: a second ask with the same key does not scan).
    private(set) var scans = 0

    func page(id: String, passage: String, letters: [String], text: String) -> PassageMark.Page? {
        let k = [id, passage, letters.joined(separator: "\u{1}"), String(text.count)].joined(separator: "\u{2}")
        if k == key { return page }
        scans += 1
        key = k; page = PassageMark.page(passage: passage, letters: letters, in: text)
        return page
    }

    func ranges(id: String, passage: String, letters: [String], text: String) -> [Range<String.Index>] {
        let k = [id, passage, letters.joined(separator: "\u{1}"), String(text.count)].joined(separator: "\u{2}")
        if k == rangesKey { return held }
        scans += 1
        rangesKey = k; held = PassageMark.ranges(passage: passage, letters: letters, in: text)
        return held
    }
}
