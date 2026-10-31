package fyi.nnx.needle

import fyi.nnx.needle.ui.lyricLayout
import fyi.nnx.needle.ui.textWidth
import fyi.nnx.needle.ui.wrapLyric
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** The lyric card's layout, as on the computers (crates/needle/src/ui/lyric_card.rs). */
class LyricCardLayoutTest {
    @Test
    fun wordsWrapAtSpaces() {
        // The computers' own cases.
        assertEquals(listOf("I keep a", "lamp burning", "for you"), wrapLyric("I keep a lamp burning for you", 12))
        assertTrue(wrapLyric("   ", 10).isEmpty())
        assertEquals(listOf("사랑해", "요", "love"), wrapLyric("사랑해요 love", 6))
        assertEquals(listOf("abc", "def", "gh"), wrapLyric("abcdefgh", 3))
    }

    @Test
    fun wideLettersCountTwiceAndWrapBetweenLetters() {
        assertEquals(4, textWidth("夜空"))
        val rows = wrapLyric("夜空に光る星を見て", 6)
        assertTrue(rows.all { textWidth(it) <= 6 })
        assertEquals("夜空に光る星を見て", rows.joinToString(""))
    }

    @Test
    fun aWordTooLongForARowIsCut() {
        assertTrue(wrapLyric("supercalifragilistic", 8).all { it.length <= 8 })
    }

    @Test
    fun shortLinesAreLargerAndLongOnesGetMoreRows() {
        val (short, one) = lyricLayout("Hold on")
        assertEquals(104, short)
        assertEquals(1, one.size)
        val (long, many) = lyricLayout("the tide is turning slow ".repeat(8).trim())
        assertTrue(long < short && many.size > 4)
        assertTrue(many.size <= 12 && long >= 52)
    }
}
