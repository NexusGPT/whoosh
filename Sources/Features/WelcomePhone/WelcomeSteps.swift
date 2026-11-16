import SwiftUI

/// The step on screen. One small view per step; the words are the catalogue's (`Words.text`), the controls come from `WelcomeControls`.
struct WelcomeStepHost: View {
    let flow: WelcomePhoneFlow
    let m: AppModel

    var body: some View {
        Group {
            switch flow.step {
            case .first: first
            case .scanning: WelcomeScanStep(flow: flow, m: m)
            case .refused: refused
            case .arriving: WelcomeArrivingStep(flow: flow, m: m)
            case .notify: notify
            case .noMac: noMac
            case .done: EmptyView()
            }
        }
        .id(flow.step)
        .transition(.opacity)
    }

    private var first: some View {
        WelcomeStepFrame(title: Words.text("Scan the code from your Mac"),
                         line: Words.text("On your Mac, open Whoosh and choose Move accounts to iPhone. Your accounts arrive here, with no password to type."),
                         art: { WelcomeMark() },
                         foot: {
            WelcomeMainButton(control: WelcomeControls.scan) { flow.start() }
            WelcomeTextButton(control: WelcomeControls.noMac) { flow.noMacYet() }
        })
    }

    private var refused: some View {
        WelcomeStepFrame(title: Words.text("The code did not work"), line: flow.refusal?.words ?? "",
                         art: { WelcomeMark() },
                         foot: {
            WelcomeMainButton(control: WelcomeControls.scanAgain) { flow.start() }
            WelcomeTextButton(control: WelcomeControls.noMac) { flow.noMacYet() }
        })
    }

    private var notify: some View {
        WelcomeStepFrame(title: Words.text("Notifications"),
                         line: Words.text("Whoosh tells you when new mail arrives, on this phone only."),
                         art: { WelcomeMark() },
                         foot: {
            WelcomeMainButton(control: WelcomeControls.allow) { flow.answerNotify(allow: true, m) }
            WelcomeTextButton(control: WelcomeControls.notNow) { flow.answerNotify(allow: false, m) }
        })
    }

    private var noMac: some View {
        WelcomeStepFrame(title: Words.text("No Mac yet"),
                         line: Words.text("Install Whoosh on a Mac and add your accounts there. Then come back here and scan its code."),
                         art: { WelcomeMark() },
                         foot: { WelcomeMainButton(control: WelcomeControls.scan) { flow.start() } })
    }
}

/// The camera step: the system scanner inside a frame with four corner brackets. A test copy shows the frame with no camera.
struct WelcomeScanStep: View {
    let flow: WelcomePhoneFlow
    let m: AppModel

    var body: some View {
        WelcomeStepFrame(title: Words.text("Scanning"), line: Words.text("Hold the code from your Mac inside the frame."),
                         art: { viewfinder },
                         foot: { WelcomeTextButton(control: WelcomeControls.cancel) { flow.back() } })
    }

    private var viewfinder: some View {
        let shape = RoundedRectangle(cornerRadius: WelcomeMetrics.frameRadius, style: .continuous)
        return ZStack {
            shape.fill(PhoneGround.trackFill)
            camera
            WelcomeBrackets()
                .stroke(WhooshPalette.label, style: StrokeStyle(lineWidth: WelcomeMetrics.frameStroke, lineCap: .round, lineJoin: .round))
        }
        .frame(width: WelcomeMetrics.frameSide, height: WelcomeMetrics.frameSide)
        .clipShape(shape)
    }

    @ViewBuilder private var camera: some View {
        #if os(iOS) && canImport(VisionKit)
        if WelcomeTestCopy.on {
            Image(systemName: "qrcode.viewfinder").font(.system(size: WelcomeMetrics.frameSide / 3, weight: .light))
                .foregroundStyle(WhooshPalette.tertiaryLabel)
        } else {
            WelcomeScannerView(onCode: { flow.scanned($0, m) }, onFail: { flow.refuse(.noCamera) })
        }
        #else
        Image(systemName: "qrcode.viewfinder").font(.system(size: WelcomeMetrics.frameSide / 3, weight: .light))
            .foregroundStyle(WhooshPalette.tertiaryLabel)
        #endif
    }
}

/// The accounts arriving: each with its real mark, name and colour, in the Mac's order, with a count. Continue never waits for the list.
struct WelcomeArrivingStep: View {
    let flow: WelcomePhoneFlow
    let m: AppModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let line = flow.complete
            ? Words.text("%lld accounts, no password asked", flow.total)
            : Words.text("%lld of %lld accounts", flow.arrived.count, flow.total)
        WelcomeStepFrame(title: Words.text("Your accounts"), line: line, middle: AnyView(list),
                         art: { WelcomeMark(size: WelcomeMetrics.markSmall) },
                         foot: { WelcomeMainButton(control: WelcomeControls.next) { flow.toNotify(m) } })
    }

    private var list: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 0) {
                    ForEach(Array(flow.arrived.enumerated()), id: \.element.id) { i, a in
                        WelcomeAccountRow(account: a, index: i)
                            .transition(reduceMotion ? .opacity : .opacity.combined(with: .offset(y: WelcomeMetrics.arrivalRise)))
                    }
                }
                .animation(reduceMotion ? nil : WhooshMotion.newRow, value: flow.arrived.count)
            }
            .scrollIndicators(.hidden)
            .onChange(of: flow.arrived.count) { _, _ in
                if let last = flow.arrived.last { proxy.scrollTo(last.id, anchor: .bottom) }
            }
        }
        .accessibilityIdentifier("iphone-welcome.accounts")
    }
}

/// One arrived account: its mark (ring in its colour, its icon), its name and its address; a hairline under it.
struct WelcomeAccountRow: View {
    let account: PhoneAccount
    let index: Int

    var body: some View {
        HStack(spacing: WhooshSpacing.s12) {
            PhoneTile(account: account, small: true)
            VStack(alignment: .leading, spacing: 0) {
                Text(account.name).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Text(account.address).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, PhoneMetrics.gutter).padding(.vertical, WhooshSpacing.s8)
        .frame(height: PhoneMetrics.accountRow)
        .overlay(alignment: .bottom) {
            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator / 2)
                .padding(.leading, PhoneMetrics.gutter + PhoneMetrics.tileSmall + WhooshSpacing.s12)
        }
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("iphone-welcome.accounts.\(index)")
    }
}
