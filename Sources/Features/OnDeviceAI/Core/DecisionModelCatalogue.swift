import Foundation

/// A family of decision models: the models of one family load and answer through one adapter (SPEC A133: "one adapter per family").
public enum DecisionModelFamily: String, Sendable, CaseIterable {
    /// Laya (convaiinnovations/laya): an encoder with a typed-decisions head; the app's engine is `LayaEngine`.
    case laya
    /// A label reader: a language model that reads the state and one question with lettered options, and answers with the option's letter
    /// (PelaAI KnowLine on a Qwen 3.5 base); the app's engine is `LabelReaderEngine`, its request text `LabelReaderLayout`.
    case labelReader
    /// GLiDE (fastino/GLiDE on a Qwen 3.5 base): a text model that reads one prompt per question and whose answer is read from a 255-way
    /// readout at the prompt's last token; the app's engine is `GlideEngine`, its request text `GlideLayout`.
    case glide
}

/// One decision model the app can run on this Mac (SPEC A133): what Settings, Decisions lists on its line, and what its download reads.
/// Every field is filled for every line: `checks/decision-models-check.py` fails on a line that lacks one.
public struct DecisionModelEntry: Equatable, Sendable, Identifiable {
    /// The id kept in the saved decision model (`DecisionModel.model`) and named by decided-by. Never shown.
    public let id: String
    /// The name the list shows.
    public let name: String
    /// The model's name in the index table (research/decision-models-on-device.md): the row its score is read from.
    public let indexRow: String
    public let family: DecisionModelFamily
    /// The model's size in billions of parameters, as its authors state it.
    public let parametersBillions: Double
    /// What the download weighs, in MB (docs/CONSTANTS.md holds the number once).
    public let downloadMB: Int
    /// The licence of the weights, as their page states it.
    public let licence: String
    /// The page the licence and the weights are read from.
    public let page: String
    /// The index's Full score (its main score: private tests, 0 is random guessing, 100 is perfect) and the date of the index table it is read
    /// from (research/decision-models-on-device.md holds the table and its source). Both or neither: a line without them shows no score.
    public let score: Double?
    public let scoreEdition: String?
    /// Where the files are fetched from, the files, and the folder they land in under the models folder.
    public let base: URL
    public let files: [String]
    /// The size in bytes and the SHA-256 of each file, for a download that is checked against them (`ModelInstaller`); empty for a download
    /// checked by size alone (Laya's, as before this table). When filled, it names the files of `files`, in their order.
    public let checked: [ModelFile]
    public let folder: String
    /// The folder, inside `folder`, that holds the checkpoint the engine loads; empty when it is `folder` itself.
    public let checkpoint: String

    /// The pinned download of a line whose files are checked against their checksums; nil for Laya.
    public var spec: MlxModelSpec? {
        checked.isEmpty ? nil : MlxModelSpec(name: name, licence: licence, baseURL: base, files: checked, folderName: folder)
    }
}

public enum DecisionModelCatalogue {
    /// Laya: the model the app has shipped with since the first on-device sort. Apache-2.0 weights; the files and the address are the ones
    /// `LayaModelStore` downloaded before this table existed (it now reads them from here).
    public static let laya = DecisionModelEntry(
        id: "laya",
        name: "Laya",
        indexRow: "Laya",
        family: .laya,
        parametersBillions: 0.42,
        downloadMB: WhooshConstants.DECISION_MODEL_LAYA_MB,
        licence: "Apache-2.0",
        page: "https://huggingface.co/convaiinnovations/laya",
        score: 9.21,
        scoreEdition: "2026-11-12",
        base: URL(string: "https://huggingface.co/convaiinnovations/laya/resolve/main/")!,
        files: [
            "typed-decisions/rl_agent_config.json",
            "typed-decisions/encoder/config.json",
            "typed-decisions/tokenizer/tokenizer.json",
            "typed-decisions/tokenizer/tokenizer_config.json",
            "typed-decisions/model.safetensors",
        ],
        checked: [],
        folder: "laya",
        checkpoint: "typed-decisions")

    /// KnowLine 0.8B Gen1 (PelaAI, Apache-2.0, on Qwen 3.5 0.8B): a label reader. The eight files the engine reads, at a fixed revision, with
    /// the sizes and checksums its repository publishes (SHA256SUMS). Proven on made-up mail against its authors' own front end
    /// (checks/corpora/label-reader/reference.json): token ids identical, answers within the weights' rounding. About 1.9 GB of memory while it sorts.
    public static let knowline08 = DecisionModelEntry(
        id: "knowline-0.8b",
        name: "KnowLine 0.8B",
        indexRow: "KnowLine 0.8B Gen1",
        family: .labelReader,
        parametersBillions: 0.85,
        downloadMB: WhooshConstants.DECISION_MODEL_KNOWLINE_08_MB,
        licence: "Apache-2.0",
        page: "https://huggingface.co/PelaAI/KnowLine-0.8B-Gen1",
        score: 29.24,
        scoreEdition: "2026-11-12",
        base: URL(string: "https://huggingface.co/PelaAI/KnowLine-0.8B-Gen1/resolve/01952e55d70c9cc087db25656cafb125590ea4a6/")!,
        files: [
            "config.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "vocab.json",
            "merges.txt",
            "chat_template.jinja",
            "temperatures.json",
            "model.safetensors",
        ],
        checked: [
            ModelFile(path: "config.json", bytes: 2907, sha256: "b90b86f35c8e6925ef74ee04d0e758f0a845c83a42089ad82bbaa948de9b4204"),
            ModelFile(path: "tokenizer.json", bytes: 12_807_982, sha256: "5f9e4d4901a92b997e463c1f46055088b6cca5ca61a6522d1b9f64c4bb81cb42"),
            ModelFile(path: "tokenizer_config.json", bytes: 16_709, sha256: "49e2b6e395f959f077f1e992b338919c0d4a9732fc6e613995e06557f843500c"),
            ModelFile(path: "vocab.json", bytes: 6_722_759, sha256: "ce99b4cb2983d118806ce0a8b777a35b093e2000a503ebde25853284c9dfa003"),
            ModelFile(path: "merges.txt", bytes: 3_353_259, sha256: "a9d356d7bdf1ef4949e3e748e95b8e10ad9d4e2e838eddc38a0a7b6b94d1db8d"),
            ModelFile(path: "chat_template.jinja", bytes: 7755, sha256: "273d8e0e683b885071fb17e08d71e5f2a5ddfb5309756181681de4f5a1822d80"),
            ModelFile(path: "temperatures.json", bytes: 17, sha256: "aef62724291c261df3b42c6f51d22b11abe6515bd87adf077d55a042d5fb0283"),
            ModelFile(path: "model.safetensors", bytes: 1_706_035_272, sha256: "433818210672e99e186f2f874a19568f4337a8714aeafd6b5c86a3ed83b3be0f"),
        ],
        folder: "knowline-0.8b-gen1/01952e55",
        checkpoint: "")

    /// GLiDE 0.8B (Fastino, on Qwen 3.5 0.8B): the seven files the engine reads, at a fixed revision, with their sizes and SHA-256 (computed
    /// from the files fetched at that revision on 15 Nov 2026; the weights' and the tokenizer's equal the ones the repository publishes).
    /// Proven on made-up mail against its authors' own code (checks/corpora/glide): the prompt byte for byte, the answers within 0.04 with
    /// the published 16-bit weights. About 1.9 GB of memory while it sorts. Its licence is the Fastino Community License, not an open one:
    /// commercial use must show "Built with Fastino GLiDE", and production commercial use by an entity over 10,000,000 US dollars of yearly
    /// revenue needs a written agreement. So the line is built and measured, and HELD (`held`): it joins the list on the owner's word.
    public static let glide08 = DecisionModelEntry(
        id: "glide-0.8b",
        name: "GLiDE 0.8B",
        indexRow: "GLiDE 0.8B",
        family: .glide,
        parametersBillions: 0.75,
        downloadMB: WhooshConstants.DECISION_MODEL_GLIDE_08_MB,
        licence: "Fastino Community License",
        page: "https://huggingface.co/fastino/GLiDE-0.8B",
        score: 36.36,
        scoreEdition: "2026-11-12",
        base: URL(string: "https://huggingface.co/fastino/GLiDE-0.8B/resolve/9eacdeec79fa48ec52a8d6f54b331376b4bbfaf0/")!,
        files: [
            "config.json",
            "decision_config.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "model.safetensors.index.json",
            "readout.safetensors",
            "model-00001-of-00001.safetensors",
        ],
        checked: [
            ModelFile(path: "config.json", bytes: 2907, sha256: "b90b86f35c8e6925ef74ee04d0e758f0a845c83a42089ad82bbaa948de9b4204"),
            ModelFile(path: "decision_config.json", bytes: 4351, sha256: "9b3e416bbcaf8eb180b76a1b83706d679777d9df159f04ddda0973451dcdf002"),
            ModelFile(path: "tokenizer.json", bytes: 19_989_325, sha256: "06b9509352d2af50381ab2247e083b80d32d5c0aba91c272ca9ff729b6a0e523"),
            ModelFile(path: "tokenizer_config.json", bytes: 1124, sha256: "66e427c470fe580fe8c7b5725d857af23d8417e37fae62667ec698306a19987b"),
            ModelFile(path: "model.safetensors.index.json", bytes: 30_846, sha256: "e317f1c88e830e95dcd2841fe0b9284fcc92a813b4ac10391dc4acfcbce027bf"),
            ModelFile(path: "readout.safetensors", bytes: 522_320, sha256: "063bbfcc74526c02bd9112698e62012739155c051155675be07721627f3beb20"),
            ModelFile(path: "model-00001-of-00001.safetensors", bytes: 1_504_827_608, sha256: "9c69993e877d713d6daf175681d56c1bdc2524eb8753f0ec6d3772f81bcb107a"),
        ],
        folder: "glide-0.8b/9eacdeec",
        checkpoint: "")

    /// Every model the list shows, in its order. The first is the default.
    public static let all: [DecisionModelEntry] = [laya, knowline08]

    /// The models the app can run that the list does not show: each waits for the owner's word (GLiDE: its licence). A test copy that
    /// names one in `WHOOSH_RIG_HELD_MODELS` lists it, so the checks can run it; the owner's copy never does.
    public static let held: [DecisionModelEntry] = [glide08]

    /// The model a saved choice names: a nil or unknown id is the default (a choice saved before this table carries no id).
    public static func entry(id: String?) -> DecisionModelEntry { all.first { $0.id == id } ?? all[0] }
}
