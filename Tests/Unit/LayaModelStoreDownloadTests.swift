import Foundation
import Testing
@testable import WhooshUI

/// The decision model's download against a loopback server (no real network): progress at the click, moving figures inside a file, ready with the
/// summed size, a 404 naming its file, a retry that works, a second click that joins the running download.
/// Mutants (each must turn this red; run by _captures/settingsaimac-r4/laya-download/run.sh): the timer loop removed, the start state removed.
@Suite("LayaModelStoreDownload") struct LayaModelStoreDownloadTests {
    @Test func downloadShowsProgressFailsInWordsAndRetries() async {
        let fails = await LayaDownloadScenario.run()
        #expect(fails.isEmpty, "\(fails)")
    }
}
