package com.elumine.searchdeadcode.fixes

import com.elumine.searchdeadcode.baseline.BaselineWriter
import com.elumine.searchdeadcode.findings.SdcFindingsStore
import com.elumine.searchdeadcode.notify.SdcNotifier
import com.elumine.searchdeadcode.sarif.SarifParser
import com.elumine.searchdeadcode.sarif.SdcFinding
import com.intellij.codeInsight.daemon.DaemonCodeAnalyzer
import com.intellij.openapi.command.WriteCommandAction
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.LocalFileSystem
import com.intellij.openapi.vfs.VfsUtil
import com.intellij.psi.PsiFile
import java.nio.file.Paths

/**
 * Appends the finding to `.deadcode-baseline.json` — the conventional file
 * `--profile ci` picks up, in the shape the CLI accepts. Only constructed
 * when the message yielded a (kind, name): an invented entry would never
 * match on the CLI side and would silently rot in the file.
 */
class AddToBaselineFix(path: String, finding: SdcFinding) : SdcFixBase(path, finding) {

    override fun getText(): String = "Add to searchdeadcode baseline (${finding.ruleId})"

    override fun invoke(project: Project, editor: Editor?, file: PsiFile?) {
        val basePath = project.basePath ?: return
        // lowercased and validated against the CLI's kind vocabulary — the
        // message capitalizes ("Parameter '…'"), matches() compares lowercase
        val key = SarifParser.baselineKey(finding) ?: return
        val relativeFile = try {
            Paths.get(basePath).relativize(Paths.get(path)).toString().replace('\\', '/')
        } catch (_: IllegalArgumentException) {
            path
        }
        val entry = BaselineWriter.Issue(
            file = relativeFile,
            name = key.second,
            kind = key.first,
            line = finding.line + 1,
            rule = finding.ruleId,
        )

        val baselineDir = LocalFileSystem.getInstance().refreshAndFindFileByNioFile(Paths.get(basePath))
            ?: return
        WriteCommandAction.runWriteCommandAction(project, text, null, {
            val existing = baselineDir.findChild(BaselineWriter.FILE_NAME)
                ?.let { String(it.contentsToByteArray(), Charsets.UTF_8) }
            val next = BaselineWriter.append(existing, entry)
            val target = baselineDir.findChild(BaselineWriter.FILE_NAME)
                ?: baselineDir.createChildData(this, BaselineWriter.FILE_NAME)
            VfsUtil.saveText(target, next)
        })

        SdcFindingsStore.getInstance(project).remove(path, finding)
        file?.let { DaemonCodeAnalyzer.getInstance(project).restart(it) }
        SdcNotifier.info(
            project,
            "Added to ${BaselineWriter.FILE_NAME} — the CI profile picks it up on its own.",
        )
    }
}
