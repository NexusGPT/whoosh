import Foundation

/// The three writes of the inspector, each through the core's own door, and what is read back to show they landed.
extension LabelInspectorModel {
    /// Rule text still being typed counts as a value once the label is written.
    private func settled() -> LabelDraft {
        var d = draft
        d.entry = d.entry.settled()
        d.flow.others = d.flow.others.map { $0.settled() }
        d.flow.exits = d.flow.exits.map { $0.settled() }
        return d
    }

    private func word(_ e: Error) -> String {
        if case CoreError.Invalid(_, let words) = e { return Words.text(words) }
        if case CoreError.NotFound = e { return LabelsWords.gone }
        return "\(e)"
    }

    /// Create (↩ on a new label): `createLabelFull`. The inspector then shows the label it made, as a modify of it.
    @discardableResult
    func create() -> Bool {
        guard isNew, canWrite else { return false }
        let d = settled()
        do {
            let out = try core.run(command: .createLabelFull(spec: d.spec))
            guard let id = out.createdId else { failure = Words.text("The core made no label."); return false }
            mode = .modify(id); failure = nil
            statusLine = Words.text("Created · new mail follows its rules")
            do { try writeFlow(id, d, was: nil) } catch { failure = word(error) }
            land(id)
            return true
        } catch {
            failure = word(error); return false
        }
    }

    /// Apply (↩ on a changed label): `updateLabel` replaces every field. A moved meaning or rule then asks the Re-run question.
    @discardableResult
    func apply() -> Bool {
        guard let id = labelId, canWrite else { return false }
        let d = settled(), baseline = saved, n = changes, moved = d.filingMoved(from: saved)
        do {
            _ = try core.run(command: .updateLabel(labelId: id, spec: d.spec))
            failure = nil
            statusLine = n == 1 ? Words.text("1 change applied · new mail follows it") : Words.text("%lld changes applied · new mail follows them", n)
            do { try writeFlow(id, d, was: baseline.flow) } catch { failure = word(error) }
            land(id)
            if moved {
                // the number of messages a re-run would look at: those filed, those its rules catch, and (AI card on, described) the newest inbound mail the model judges
                offer = Offer(filed: Int(core.state(path: "label/\(id)/rerun/candidates") ?? "") ?? Int(core.labelRerunCount(labelId: id)), baseline: baseline, join: 0, leave: 0, stay: 0, beyond: Int(core.state(path: "label/\(id)/rerun/beyond") ?? "") ?? 0)
                recompute()   // `land` computed the proof before the offer existed: the core's own preview (`label/<id>/rerun/preview`) is read once the offer is there
            }
            return true
        } catch {
            failure = word(error); return false
        }
    }

    /// The offer line reads the preview's own counts once the proof has landed ("2 join Work, 1 leaves, 3 stay").
    func settleOffer() {
        guard var o = offer else { return }
        o.join = proof.joinCount; o.leave = proof.leaveCount; o.stay = proof.stayCount
        offer = o
    }

    /// Re-run (R): `rerunLabel` answers at once and the core judges in the background (a model takes its own time, no view waits for it); the
    /// progress and the result are read from `label/<id>/rerun/*` as they land: how many were filed, re-filed and unfiled, and who decided.
    @discardableResult
    func rerun() -> Bool {
        guard let id = labelId, offer != nil else { return false }
        // the preview's join group is read before the write; when the proof has not landed yet it is planned again here, from the rules the label had
        var joined = proof.join.map(\.id)
        if joined.isEmpty, !corpus.isEmpty {
            let filed = Set(corpus.filter { $0.row.labelNames.contains(saved.name) }.map(\.row.messageId))
            joined = LabelProofCompute.compute(core: core, corpus: corpus, draft: draft, saved: offer?.baseline ?? saved, filed: filed).join.map(\.id)
        }
        do {
            _ = try core.run(command: .rerunLabel(labelId: id))
        } catch { failure = word(error); return false }
        offer = nil
        watchRerun(id, joined: joined)
        return true
    }

    /// Reads the re-run's state until it ends (off the main actor between reads), shows its progress, then its result.
    func watchRerun(_ id: String, joined: [String]) {
        rerunning = true
        statusLine = Words.text("Sorting…")
        Task { [weak self] in
            while true {
                guard let self else { return }
                let (state, done, total) = await MainActor.run { (self.core.state(path: "label/\(id)/rerun/state") ?? "", self.core.state(path: "label/\(id)/rerun/done") ?? "0", self.core.state(path: "label/\(id)/rerun/total") ?? "0") }
                if !state.contains("running") { break }
                await MainActor.run { self.statusLine = Words.text("Sorted %@ of %@", done, total) }
                try? await Task.sleep(nanoseconds: 150_000_000)
            }
            await MainActor.run { [weak self] in self?.finishRerun(id, joined: joined) }
        }
    }

    private func finishRerun(_ id: String, joined: [String]) {
        func read(_ k: String) -> Int { Int(core.state(path: "label/\(id)/rerun/\(k)") ?? "") ?? 0 }
        let decided = joined.map { m -> (messageId: String, by: String, card: String?, chain: Int?) in
            let by = (core.state(path: "message/\(m)/label/\(id)/decided_by") ?? "").trimmingCharacters(in: CharacterSet(charactersIn: "\""))
            let why = StoredRules.object(core.state(path: "message/\(m)/why/\(id)")) as? [String: Any]
            return (messageId: m, by: by, card: why?["card"] as? String, chain: (why?["chain_id"] as? NSNumber)?.intValue)
        }
        result = RerunResult(filed: read("filed"), refiled: read("refiled"), unfiled: read("unfiled"), decided: decided, judged: read("done"), total: read("total"))
        rerunning = false
        statusLine = Words.text("Re-filed %lld messages", result?.refiled ?? 0)
        land(id)
        // a step stops at 500 messages and says so: the offer comes back with the number left, and Re-run (R) sorts the next ones
        if read("beyond") > 0 {
            offer = Offer(filed: read("candidates"), baseline: saved, join: 0, leave: 0, stay: 0, beyond: read("beyond"))
        }
    }

    /// Not now (Esc on the offer): today's filing stays as it is; the inspector stays open.
    @discardableResult
    func notNow() -> Bool {
        guard offer != nil else { return false }
        offer = nil; recompute()
        return true
    }

    /// After a write the screen shows what the core holds now, never what was sent: the label, its chains and cards, the foot's figures, the proof.
    private func land(_ id: String) {
        app.refresh()
        if let l = core.labels().first(where: { $0.id == id }) { let d = readDraft(l); draft = d; saved = d }
        refreshCounts(); refreshWhy()
        corpus = LabelProofCompute.corpus(app); recompute()
    }
}
