package app.operit

import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * JVM unit tests for the hand-off that happens *after* a firing was consumed from
 * storage — the three paths named in the upstream review: broadcast completion,
 * background submission failure and delivery failure.
 *
 * The hand-off is a pure object with injected side effects, so these run without an
 * Android runtime. What still needs a device is the receiver wiring around it
 * (goAsync / runBackground / logcat), and is not covered here.
 */
class HostEventHandoffTest {
    private val schedule = AndroidHostEventSchedule(
        scheduleId = "hook-1",
        kind = "timer",
        delayMs = 1000L,
        intervalMs = null,
        nextFireAtMillis = 1100L,
    )
    private val firedAtMillis = 5000L
    private val retained = mutableListOf<UndeliveredHostEventFire>()

    // --- path 1: the broadcast completes: submitted, delivered, nothing retained ---

    @Test
    fun broadcastCompletionDeliversWithoutRetaining() {
        var started = 0
        var submitted = 0
        var emitted = 0

        val beginOutcome = HostEventHandoff.begin(
            schedule = schedule,
            firedAtMillis = firedAtMillis,
            startCore = { started++ },
            submitBackground = { _, _ -> submitted++ },
            retain = { retained.add(it) },
        )
        val deliveryOutcome = HostEventHandoff.completeDelivery(
            schedule = schedule,
            firedAtMillis = firedAtMillis,
            emit = { emitted++ },
            retain = { retained.add(it) },
        )

        assertEquals(HostEventHandoffOutcome.Submitted, beginOutcome)
        assertEquals(HostEventHandoffOutcome.Delivered, deliveryOutcome)
        assertEquals(1, started)
        assertEquals(1, submitted)
        assertEquals(1, emitted)
        assertTrue(retained.isEmpty())
    }

    // --- path 2: background submission failures are retained, exactly once, no retry ---

    @Test
    fun coreStartFailureIsRetainedAsStartCore() {
        val outcome = HostEventHandoff.begin(
            schedule = schedule,
            firedAtMillis = firedAtMillis,
            startCore = { throw IllegalStateException("core service unavailable") },
            submitBackground = { _, _ -> throw AssertionError("must not submit after a start failure") },
            retain = { retained.add(it) },
        )

        assertTrue(outcome is HostEventHandoffOutcome.Undelivered)
        val fire = (outcome as HostEventHandoffOutcome.Undelivered).fire
        assertEquals(HostEventHandoffStage.START_CORE, fire.stage)
        assertEquals("hook-1", fire.scheduleId)
        assertEquals(firedAtMillis, fire.firedAtMillis)
        assertTrue(fire.reason.contains("core service unavailable"))
        assertEquals(1, retained.size)
    }

    @Test
    fun backgroundSubmitFailureIsRetainedOnceWithoutRetry() {
        var submitCalls = 0

        val outcome = HostEventHandoff.begin(
            schedule = schedule,
            firedAtMillis = firedAtMillis,
            startCore = {},
            submitBackground = { _, _ ->
                submitCalls++
                throw IllegalStateException("bridge not ready")
            },
            retain = { retained.add(it) },
        )

        assertTrue(outcome is HostEventHandoffOutcome.Undelivered)
        val undelivered = outcome as HostEventHandoffOutcome.Undelivered
        assertEquals(HostEventHandoffStage.SUBMIT_BACKGROUND, undelivered.fire.stage)
        assertTrue(undelivered.fire.reason.contains("bridge not ready"))
        assertEquals(1, submitCalls)
        assertEquals(1, retained.size)
    }

    // --- path 3: delivery failures are retained instead of only logged ---

    @Test
    fun coreRejectionAnswerIsRetainedAsDelivery() {
        val outcome = HostEventHandoff.completeDelivery(
            schedule = schedule,
            firedAtMillis = firedAtMillis,
            emit = { check(false) { "Core rejected Android host event schedule" } },
            retain = { retained.add(it) },
        )

        assertTrue(outcome is HostEventHandoffOutcome.Undelivered)
        val undelivered = outcome as HostEventHandoffOutcome.Undelivered
        assertEquals(HostEventHandoffStage.DELIVERY, undelivered.fire.stage)
        assertTrue(undelivered.fire.reason.contains("Core rejected"))
        assertTrue(undelivered.cause is IllegalStateException)
        assertEquals(1, retained.size)
    }

    @Test
    fun thrownEmitErrorIsRetainedAsDelivery() {
        val nativeError = RuntimeException("native emit failed")

        val outcome = HostEventHandoff.completeDelivery(
            schedule = schedule,
            firedAtMillis = firedAtMillis,
            emit = { throw nativeError },
            retain = { retained.add(it) },
        )

        assertTrue(outcome is HostEventHandoffOutcome.Undelivered)
        val undelivered = outcome as HostEventHandoffOutcome.Undelivered
        assertEquals(HostEventHandoffStage.DELIVERY, undelivered.fire.stage)
        assertSame(nativeError, undelivered.cause)
        assertEquals(1, retained.size)
    }

    // --- the retained record survives as JSON (what gets written back) ---

    @Test
    fun retainedRecordEncodesStageAndReason() {
        val fire = UndeliveredHostEventFire(
            scheduleId = "hook-1",
            firedAtMillis = firedAtMillis,
            stage = HostEventHandoffStage.DELIVERY,
            reason = "boom",
        )

        val json = fire.toJson()

        assertEquals("hook-1", json.getString("scheduleId"))
        assertEquals(firedAtMillis, json.getLong("firedAtMillis"))
        assertEquals("DELIVERY", json.getString("stage"))
        assertEquals("boom", json.getString("reason"))
    }

    // --- Throwable, not only Exception: the firing is already consumed when these run,
    //     so an escaping Error must still be retained (and finish() still guaranteed). ---

    @Test
    fun coreStartErrorIsRetainedInsteadOfEscaping() {
        val failure = Error("core start exploded")

        val outcome = HostEventHandoff.begin(
            schedule = schedule,
            firedAtMillis = firedAtMillis,
            startCore = { throw failure },
            submitBackground = { _, _ -> throw AssertionError("must not submit after a start failure") },
            retain = { retained.add(it) },
        )

        assertTrue(outcome is HostEventHandoffOutcome.Undelivered)
        val undelivered = outcome as HostEventHandoffOutcome.Undelivered
        assertEquals(HostEventHandoffStage.START_CORE, undelivered.fire.stage)
        assertSame(failure, undelivered.cause)
        assertEquals(1, retained.size)
    }
}
