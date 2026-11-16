import SwiftUI
#if canImport(AppKit)
import AppKit
import QuartzCore
#else
import UIKit
#endif

/// The one rich text editor both composers use. The SwiftUI surface is platform-neutral; underneath
/// sits an AppKit text view on the Mac and a UIKit one on the iPhone. All text lives in `session`.
public struct RichTextEditor: View {
    let session: RichTextSession
    public init(session: RichTextSession) { self.session = session }

    #if canImport(AppKit)
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    public var body: some View {
        ZStack(alignment: .topLeading) {
            PlatformRichText(session: session)
                .accessibilityIdentifier("mac-composer.body")
            if let r = session.selectionRect {
                RichTextToolbar(session: session)
                    .fixedSize()
                    .position(x: max(r.midX, WhooshSpacing.s32 * 5), y: r.minY > WhooshSpacing.s32 + WhooshSpacing.s8 ? r.minY - WhooshSpacing.s20 : r.maxY + WhooshSpacing.s24)   // below the selection when it is on the first line
                    .transition(reduceMotion ? .opacity
                        : .scale(scale: WhooshConstants.POPOVER_SCALE_FROM, anchor: .bottom).combined(with: .opacity))
            }
        }
        // Appearing and vanishing animate; following a drag does not (the value is presence, not position).
        .animation(.easeOut(duration: Double(WhooshConstants.POPOVER_MS) / 1000), value: session.selectionRect != nil)
    }
    #else
    public var body: some View {
        PlatformRichText(session: session).accessibilityIdentifier("mac-composer.body")
    }
    #endif
}

#if canImport(AppKit)

/// NSTextView with the composer's keys: ⌘Z takes an AI edit back first, ⌘B bolds, paste matches style.
final class WhooshTextView: NSTextView {
    weak var session: RichTextSession?

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        // ⇧⌘Z exactly: the composer's instant send. Shift is read from the flags, never from the letter, so ⌘Z can never send
        // (main95: ⌘Z sent the reply; a hidden SwiftUI button for ⇧⌘Z took the plain ⌘Z).
        if event.modifierFlags.intersection(.deviceIndependentFlagsMask) == [.command, .shift],
           event.charactersIgnoringModifiers?.lowercased() == "z", let s = session, s.instantSend() { return true }
        guard event.modifierFlags.intersection(.deviceIndependentFlagsMask) == .command,
              let k = event.charactersIgnoringModifiers?.lowercased() else { return super.performKeyEquivalent(with: event) }
        if k == "z", let s = session, s.canUndoRewrite { return s.undoRewrite() }
        if k == "b" { session?.hooks.toggleBold(); return true }
        if k == "i" { session?.hooks.toggleItalic(); return true }
        if k == "u" { session?.hooks.toggleUnderline(); return true }
        return super.performKeyEquivalent(with: event)
    }

    override func paste(_ sender: Any?) {
        if let s = session, s.pasteFiles() { return }   // copied files become attachments, never their names in the text
        let pb = session?.pasteboardName.map { NSPasteboard(name: NSPasteboard.Name($0)) } ?? .general
        if let html = pb.string(forType: .html) {
            insertText(RichAttributed.make(RichHTML.parse(html)), replacementRange: selectedRange())
        } else if pb == NSPasteboard.general {
            super.paste(sender)
        } else {
            _ = readSelection(from: pb)   // the rig's private pasteboard: the same plain paste, never the owner's
        }
    }
}

/// The one way the model's text goes into the text view from outside the view's own editing (an AI draft landing, a rewrite
/// and its undo, a snippet, a template, dictation, the quick reply opening the full composer, the test socket's `type`).
/// The text view's undo manager holds typing entries with ranges of the text that is replaced; the next undo would ask
/// AppKit for a range that no longer exists (NSRangeException, main81 crash). So the typing entries of this view are
/// dropped first. The model keeps its own undo for outside edits (`RichUndoStack`, ⌘Z first in `WhooshTextView`), so the
/// replacement is not also registered with the view: that would make one step undoable twice.
enum RichStorageSwap {
    @MainActor static func replace(in tv: NSTextView, with new: NSAttributedString) {
        guard let st = tv.textStorage else { return }
        tv.breakUndoCoalescing()
        tv.undoManager?.removeAllActions()
        st.setAttributedString(new)
    }
}

struct PlatformRichText: NSViewRepresentable {
    let session: RichTextSession

    func makeCoordinator() -> Coordinator { Coordinator(session) }

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSTextView.scrollableTextView()
        let tv = WhooshTextView(frame: .zero, textContainer: (scroll.documentView as? NSTextView)?.textContainer)
        tv.session = session
        tv.isRichText = true; tv.allowsUndo = true; tv.drawsBackground = false
        tv.importsGraphics = false; tv.isAutomaticLinkDetectionEnabled = true
        tv.isVerticallyResizable = true; tv.autoresizingMask = [.width]
        // The typed text starts where the composer draws its placeholder and the signature under it (4 points in, 8 down). Lead, 13 Nov 2026 (pictures of a
        // new message and a forward on main157): the text sat 17 points to the right of its own placeholder and of the signature, and 4 points lower.
        tv.textContainerInset = NSSize(width: 0, height: WhooshSpacing.s8)
        tv.textContainer?.lineFragmentPadding = WhooshSpacing.s4
        tv.typingAttributes = RichAttributed.typing
        tv.delegate = context.coordinator
        scroll.documentView = tv
        scroll.drawsBackground = false
        context.coordinator.textView = tv
        context.coordinator.install()
        context.coordinator.reload()
        return scroll
    }

    func updateNSView(_ v: NSScrollView, context: Context) {
        if context.coordinator.applied != session.revision { context.coordinator.reload() }
    }

    @MainActor
    final class Coordinator: NSObject, NSTextViewDelegate, NSTextStorageDelegate {
        let session: RichTextSession
        weak var textView: WhooshTextView?
        var applied = -1
        private var following = false
        init(_ s: RichTextSession) { session = s }

        /// The composer's copy follows the editor after an Undo or a Redo too. The text view's own undo replaces characters in the storage;
        /// the copy was written only from `textDidChange` (main95 to main103: after the Edit menu's Undo the editor and the copy disagreed).
        /// Only an edit made while the undo manager undoes or redoes is followed here (typing already goes through textDidChange), once per
        /// turn, after the storage has finished editing.
        nonisolated func textStorage(_ ts: NSTextStorage, didProcessEditing editedMask: NSTextStorageEditActions, range: NSRange, changeInLength delta: Int) {
            guard editedMask.contains(.editedCharacters) else { return }
            MainActor.assumeIsolated {
                guard let um = textView?.undoManager, um.isUndoing || um.isRedoing, !following else { return }
                following = true
                DispatchQueue.main.async { [weak self] in
                    MainActor.assumeIsolated {
                        guard let self else { return }
                        self.following = false
                        guard let st = self.textView?.textStorage else { return }
                        self.session.viewDidChange(RichAttributed.read(st))
                        self.updateTrigger()
                    }
                }
            }
        }

        func reload() {
            guard let tv = textView, let st = tv.textStorage else { return }
            applied = session.revision
            // Feedback first: the new text is in place this frame; the fade only softens how it is seen.
            if session.consumeFade(), applied > 0 {
                tv.wantsLayer = true
                let t = CATransition(); t.type = .fade
                t.duration = WhooshMotion.readerCrossFade
                tv.layer?.add(t, forKey: "rewrite-fade")
            }
            RichStorageSwap.replace(in: tv, with: RichAttributed.make(session.document))
            tv.typingAttributes = RichAttributed.typing
            // Text set from outside (a snippet, a template, typing through the socket) lands at the end; a trigger it
            // ends with (`;` or `:`) opens its list. After the view update, so no state changes inside it.
            if tv.window?.firstResponder !== tv { tv.setSelectedRange(NSRange(location: st.length, length: 0)) }
            DispatchQueue.main.async { [weak self] in self?.updateTrigger() }
        }

        func textViewDidChangeSelection(_ n: Notification) {
            guard let tv = textView else { return }
            let r = tv.selectedRange()
            if r.length > 0, let inClip = rectInEditor(r) { session.selectionRect = inClip } else { session.selectionRect = nil }
            updateTrigger()
        }

        func textDidChange(_ n: Notification) {
            guard let tv = textView, let st = tv.textStorage else { return }
            session.viewDidChange(RichAttributed.read(st))
            updateTrigger()
        }

        func textView(_ textView: NSTextView, doCommandBy selector: Selector) -> Bool { command(selector) }

        func install() {
            session.hooks.toggleBold = { [weak self] in self?.toggleBold() }
            installFormatHooks()
            session.hooks.setLink = { [weak self] url in self?.setLink(url) }
            session.hooks.insertPicture = { [weak self] cid in self?.insertPicture(cid) }
            session.hooks.viewText = { [weak self] in self?.textView?.string }
            textView?.textStorage?.delegate = self   // the copy follows an Undo and a Redo (textStorage(_:didProcessEditing:...))
            NotificationCenter.default.addObserver(self, selector: #selector(pictureLoaded(_:)), name: RichPictures.didLoad, object: nil)
        }

        /// A thumbnail finished off the main thread: every run of that picture in the text swaps the placeholder for it.
        @objc func pictureLoaded(_ n: Notification) {
            guard let cid = n.userInfo?["cid"] as? String, let tv = textView, let st = tv.textStorage, let picture = RichPictures.shared.cached(cid) else { return }
            let size = RichPictures.drawnSize(width: picture.size.width, height: picture.size.height)
            st.enumerateAttribute(.attachment, in: NSRange(location: 0, length: st.length)) { value, range, _ in
                guard let att = value as? NSTextAttachment, st.attribute(RichAttributed.cidKey, at: range.location, effectiveRange: nil) as? String == cid else { return }
                att.image = picture
                att.bounds = CGRect(origin: .zero, size: size)
                st.edited(.editedAttributes, range: range, changeInLength: 0)
            }
        }

        func edit(_ range: NSRange, _ f: (NSMutableAttributedString) -> Void) {
            guard let tv = textView, let st = tv.textStorage, tv.shouldChangeText(in: range, replacementString: nil) else { return }
            st.beginEditing(); f(st); st.endEditing(); tv.didChangeText()
        }

        func toggleBold() {
            guard let tv = textView, let st = tv.textStorage else { return }
            let r = tv.selectedRange()
            if r.length == 0 {
                let on = tv.typingAttributes[RichAttributed.boldKey] == nil
                tv.typingAttributes[RichAttributed.boldKey] = on ? true : nil
                tv.typingAttributes[.font] = RichAttributed.font(bold: on)
                return
            }
            let on = st.attribute(RichAttributed.boldKey, at: r.location, effectiveRange: nil) == nil
            edit(r) { s in
                s.enumerateAttributes(in: r) { _, sub, _ in
                    if on { s.addAttribute(RichAttributed.boldKey, value: true, range: sub) } else { s.removeAttribute(RichAttributed.boldKey, range: sub) }
                    s.addAttribute(.font, value: RichAttributed.font(bold: on), range: sub)
                }
            }
        }

        func setLink(_ url: String) {
            guard let u = URL(string: url), RichHTML.safeURL(url) != nil, let tv = textView else { return }
            let r = tv.selectedRange()
            let link: [NSAttributedString.Key: Any] = [.link: u, .foregroundColor: RichAttributed.color(WhooshPalette.accent)]
            if r.length == 0 {   // nothing selected: the address goes in as the linked text
                tv.insertText(NSAttributedString(string: url, attributes: tv.typingAttributes.merging(link) { $1 }), replacementRange: r)
                return
            }
            edit(r) { $0.addAttributes(link, range: r) }
        }

        func insertPicture(_ cid: String) {
            guard let tv = textView else { return }
            let pic = RichAttributed.make(RichDocument(paragraphs: [RichParagraph(runs: [RichRun("", cid: cid)])]))
            tv.insertText(pic, replacementRange: tv.selectedRange())
        }
    }
}

#else

private struct PlatformRichText: UIViewRepresentable {
    let session: RichTextSession

    func makeCoordinator() -> Coordinator { Coordinator(session) }

    func makeUIView(context: Context) -> UITextView {
        let tv = UITextView()
        tv.allowsEditingTextAttributes = true; tv.backgroundColor = .clear
        tv.textContainerInset = UIEdgeInsets(top: WhooshSpacing.s12, left: WhooshSpacing.s16, bottom: WhooshSpacing.s12, right: WhooshSpacing.s16)
        tv.typingAttributes = RichAttributed.typing
        tv.delegate = context.coordinator
        context.coordinator.textView = tv
        context.coordinator.reload()
        return tv
    }

    func updateUIView(_ v: UITextView, context: Context) {
        if context.coordinator.applied != session.revision { context.coordinator.reload() }
    }

    @MainActor
    final class Coordinator: NSObject, UITextViewDelegate {
        let session: RichTextSession
        weak var textView: UITextView?
        var applied = -1
        init(_ s: RichTextSession) { session = s }

        func reload() {
            guard let tv = textView else { return }
            applied = session.revision
            tv.undoManager?.removeAllActions()   // typing entries point at the text that is replaced
            tv.attributedText = RichAttributed.make(session.document)
            tv.typingAttributes = RichAttributed.typing
        }

        func textViewDidChange(_ tv: UITextView) { session.viewDidChange(RichAttributed.read(tv.textStorage)) }
    }
}

#endif
