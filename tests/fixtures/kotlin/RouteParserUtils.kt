package sample.route.utils

import androidx.annotation.VisibleForTesting
import sample.core.ids.ItemUid
import sample.core.ids.PageUid

/**
 * Parses app deep-link URIs into an [RoutePageUidHolder].
 *
 * Segment layout (indexes below):
 * `[scheme]://openroute/items/[itemUid]/pages/[pageUid]/subpages/[subPageUid]`
 * Section variant (opens at the section's first page):
 * `[scheme]://openroute/items/[itemUid]/sections/[sectionId]`
 */
@Suppress("TooManyFunctions") // small, self-documenting segment validators read better than one inlined block
object RouteParserUtils {

    private const val ACTION_COMMAND_INDEX = 1
    private const val EDITION_PATH_SEGMENT = 2
    private const val EDITION_UID_INDEX = 3
    private const val PAGE_PATH_SEGMENT = 4
    private const val PAGE_UID_INDEX = 5
    private const val SUBPAGE_PATH_SEGMENT = 6
    private const val SUBPAGE_UID_INDEX = 7

    // The section variant reuses segment-4/5 with a "sections" keyword.
    private const val SECTION_PATH_SEGMENT = 4
    private const val SECTION_ID_INDEX = 5

    // Number of path segments per URI form.
    private const val SEGMENTS_EDITION_ONLY = 4
    private const val SEGMENTS_WITH_PAGE = 6
    private const val SEGMENTS_WITH_SUBPAGE = 8

    /**
     * Splits a URI into its path segments, dropping any query string.
     * `appx://sample/uri` -> `["appx", "sample", "uri"]`. Null/blank -> empty array.
     */
    @JvmStatic
    @VisibleForTesting
    fun extractUriParts(uriString: String?): Array<String> {
        uriString ?: return emptyArray()
        return uriString.substringBefore("?")
            .replace("://", "/")
            .split("/")
            .dropLastWhile { it.isEmpty() }
            .toTypedArray()
    }

    @JvmStatic
    fun extractItemUid(uriString: String?): ItemUid {
        val uriParts = extractUriParts(uriString)
        return if (uriParts.size > EDITION_UID_INDEX && isRoutePartValid(uriParts)) {
            ItemUid(uriParts[EDITION_UID_INDEX])
        } else {
            ItemUid.EMPTY
        }
    }

    @JvmStatic
    fun extractRouteAndPageUid(uriString: String?): RoutePageUidHolder {
        val uriParts = extractUriParts(uriString)
        return when {
            !isValidUriParts(uriParts) ->
                RoutePageUidHolder(ItemUid.EMPTY, PageUid.EMPTY, PageUid.EMPTY, null)

            isSectionForm(uriParts) -> RoutePageUidHolder(
                itemUid = ItemUid(uriParts[EDITION_UID_INDEX]),
                pageUid = PageUid.EMPTY,
                subPageUid = PageUid.EMPTY,
                sectionId = uriParts[SECTION_ID_INDEX].toInt(),
            )

            else -> RoutePageUidHolder(
                itemUid = ItemUid(uriParts[EDITION_UID_INDEX]),
                pageUid = if (uriParts.size < SEGMENTS_WITH_PAGE) PageUid.EMPTY else PageUid(uriParts[PAGE_UID_INDEX]),
                subPageUid = if (uriParts.size < SEGMENTS_WITH_SUBPAGE) PageUid.EMPTY else PageUid(uriParts[SUBPAGE_UID_INDEX]),
                sectionId = null,
            )
        }
    }

    @JvmStatic
    fun isValidRouteHolder(holder: RoutePageUidHolder): Boolean = ItemUid.EMPTY != holder.itemUid

    private fun isValidUriParts(uriParts: Array<String>): Boolean = when {
        uriParts.size != SEGMENTS_EDITION_ONLY &&
            uriParts.size != SEGMENTS_WITH_PAGE &&
            uriParts.size != SEGMENTS_WITH_SUBPAGE -> false
        !isActionCommandValid(uriParts) -> false
        !isRoutePartValid(uriParts) -> false
        isSectionForm(uriParts) -> isSectionPartValid(uriParts)
        else -> isPagePartValid(uriParts) && isSubpagePartValid(uriParts)
    }

    /** A 6-segment URI whose 4th segment is "sections": items/{uid}/sections/{sectionId}. */
    private fun isSectionForm(uriParts: Array<String>): Boolean =
        uriParts.size == SEGMENTS_WITH_PAGE && uriParts[SECTION_PATH_SEGMENT] == "sections"

    private fun isSectionPartValid(uriParts: Array<String>): Boolean =
        uriParts[SECTION_ID_INDEX].toIntOrNull() != null

    private fun isActionCommandValid(uriParts: Array<String>): Boolean =
        uriParts[ACTION_COMMAND_INDEX] == "openroute"

    private fun isRoutePartValid(uriParts: Array<String>): Boolean =
        uriParts[EDITION_PATH_SEGMENT] == "items" && uriParts[EDITION_UID_INDEX].contains("itemUid")

    private fun isPagePartValid(uriParts: Array<String>): Boolean =
        uriParts.size < SEGMENTS_WITH_PAGE ||
            uriParts[PAGE_PATH_SEGMENT] == "pages" && uriParts[PAGE_UID_INDEX].contains("PageUid")

    private fun isSubpagePartValid(uriParts: Array<String>): Boolean =
        uriParts.size != SEGMENTS_WITH_SUBPAGE ||
            uriParts[SUBPAGE_PATH_SEGMENT] == "subpages" && uriParts[SUBPAGE_UID_INDEX].contains("PageUid")

    /** [sectionId] is non-null only for the section variant; the route then opens at the section's first page. */
    data class RoutePageUidHolder(
        val itemUid: ItemUid,
        val pageUid: PageUid,
        val subPageUid: PageUid,
        val sectionId: Int?,
    )
}
