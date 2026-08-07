package com.elumine.searchdeadcode.annotate

import com.elumine.searchdeadcode.findings.SdcFindingsStore
import com.elumine.searchdeadcode.fixes.AddToBaselineFix
import com.elumine.searchdeadcode.fixes.DeleteDeadCodeFix
import com.elumine.searchdeadcode.fixes.IgnoreInlineFix
import com.elumine.searchdeadcode.fixes.OpenRuleDocFix
import com.elumine.searchdeadcode.sarif.SarifParser
import com.elumine.searchdeadcode.sarif.SdcFinding
import com.elumine.searchdeadcode.sarif.SdcLevel
import com.intellij.lang.annotation.AnnotationHolder
import com.intellij.lang.annotation.ExternalAnnotator
import com.intellij.lang.annotation.HighlightSeverity
import com.intellij.codeInspection.ProblemHighlightType
import com.intellij.openapi.project.DumbAware
import com.intellij.openapi.util.TextRange
import com.intellij.psi.PsiFile

/**
 * Renders the store's findings for one file. The daemon drives the lifecycle:
 * an edit re-triggers annotation, the store has already dropped the file's
 * findings by then (staleness policy), and returning null from collect wipes
 * the previous annotations. No manual highlighter management anywhere.
 *
 * DumbAware is load-bearing: ExternalToolPass runs during indexing and skips
 * non-dumb-aware annotators, which would apply an EMPTY set and wipe every
 * highlight each time Android Studio reindexes after a Gradle sync. This
 * annotator reads Document lines and the in-memory store, never the index.
 */
class SdcExternalAnnotator :
    ExternalAnnotator<SdcExternalAnnotator.Input, List<SdcFinding>>(), DumbAware {

    data class Input(val findings: List<SdcFinding>)

    override fun collectInformation(file: PsiFile): Input? {
        val virtualFile = file.virtualFile ?: return null
        val findings = SdcFindingsStore.getInstance(file.project).findingsFor(virtualFile)
        return if (findings.isEmpty()) null else Input(findings)
    }

    override fun doAnnotate(collectedInfo: Input?): List<SdcFinding> =
        collectedInfo?.findings ?: emptyList()

    override fun apply(file: PsiFile, annotationResult: List<SdcFinding>?, holder: AnnotationHolder) {
        val findings = annotationResult ?: return
        val document = file.viewProvider.document ?: return
        val path = file.virtualFile?.path ?: return

        for (finding in findings) {
            // Belt and braces: the store drops edited files, but a finding
            // must still never annotate past the end of the document.
            if (finding.line >= document.lineCount) continue
            val range = TextRange(
                document.getLineStartOffset(finding.line),
                document.getLineEndOffset(finding.line),
            )
            // Dead code is never a compile error; note-level findings stay quiet.
            val severity = when (finding.level) {
                SdcLevel.NOTE -> HighlightSeverity.INFORMATION
                else -> HighlightSeverity.WARNING
            }

            var builder = holder.newAnnotation(severity, "${finding.message} [${finding.ruleId}]")
                .range(range)
                .highlightType(ProblemHighlightType.LIKE_UNUSED_SYMBOL)
                .needsUpdateOnTyping(false)

            if (finding.fix != null) {
                builder = builder.withFix(DeleteDeadCodeFix(path, finding))
            }
            if (SarifParser.baselineKey(finding) != null) {
                builder = builder.withFix(AddToBaselineFix(path, finding))
            }
            builder = builder.withFix(IgnoreInlineFix(path, finding))
            if (finding.helpUri != null) {
                builder = builder.withFix(OpenRuleDocFix(finding.helpUri))
            }
            builder.create()
        }
    }
}
