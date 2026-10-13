package app.operit

import org.json.JSONObject

/**
 * Which hand-off step failed *after* the firing had already been consumed (deleted or
 * re-armed and persisted) by [AndroidHostEventScheduler.consumeFire].
 */
internal enum class HostEventHandoffStage {
    /** Starting [OperitCoreService] failed. */
    START_CORE,
    /** Obtaining the runtime host or submitting the background task failed. */
    SUBMIT_BACKGROUND,
    /** The background emit to Core failed, including `ok=false` answers. */
    DELIVERY,
}

/**
 * One firing that was consumed from storage but never handed to Core.
 *
 * It is written back to persistent storage so the loss survives this process and stays
 * visible to the upper layer. Retention is a *record*: no alarm is re-armed and no retry
 * is scheduled.
 */
internal data class UndeliveredHostEventFire(
    val scheduleId: String,
    val firedAtMillis: Long,
    val stage: HostEventHandoffStage,
    val reason: String,
) {
    fun toJson(): JSONObject = JSONObject()
        .put("scheduleId", scheduleId)
        .put("firedAtMillis", firedAtMillis)
        .put("stage", stage.name)
        .put("reason", reason)
}

/** Result of one hand-off; [Undelivered] is the failure shape that used to be logged away. */
internal sealed class HostEventHandoffOutcome {
    /** Consumed and submitted: delivery now completes on the background executor. */
    object Submitted : HostEventHandoffOutcome()

    /** The background emit completed successfully. */
    object Delivered : HostEventHandoffOutcome()

    /** Consumed, but never reached Core; [fire] has been passed to the retain callback. */
    data class Undelivered(val fire: UndeliveredHostEventFire, val cause: Throwable) :
        HostEventHandoffOutcome()
}

/**
 * The hand-off between "the alarm fired and was consumed" and "Core owns this firing".
 *
 * Everything here is free of Android types on purpose: the receiver wires the real side
 * effects in as lambdas, which keeps the three paths named in the upstream review —
 * broadcast completion, background submission failure and delivery failure — assertable
 * from plain JVM unit tests.
 */
internal object HostEventHandoff {
    /**
     * Starts Core and submits the background delivery for an already consumed firing.
     * A failure in either step is retained through [retain] and returned as
     * [HostEventHandoffOutcome.Undelivered]; it is never retried here.
     */
    fun begin(
        schedule: AndroidHostEventSchedule,
        firedAtMillis: Long,
        startCore: () -> Unit,
        submitBackground: (AndroidHostEventSchedule, Long) -> Unit,
        retain: (UndeliveredHostEventFire) -> Unit,
    ): HostEventHandoffOutcome {
        try {
            startCore()
        } catch (error: Exception) {
            return retainFailure(retain, schedule, firedAtMillis, HostEventHandoffStage.START_CORE, error)
        }
        try {
            submitBackground(schedule, firedAtMillis)
        } catch (error: Exception) {
            return retainFailure(retain, schedule, firedAtMillis, HostEventHandoffStage.SUBMIT_BACKGROUND, error)
        }
        return HostEventHandoffOutcome.Submitted
    }

    /**
     * Runs the emit on the background executor. A thrown error, or an `ok=false` answer
     * from Core, is retained as [HostEventHandoffStage.DELIVERY] instead of being logged only.
     */
    fun completeDelivery(
        schedule: AndroidHostEventSchedule,
        firedAtMillis: Long,
        emit: () -> Unit,
        retain: (UndeliveredHostEventFire) -> Unit,
    ): HostEventHandoffOutcome {
        try {
            emit()
        } catch (error: Throwable) {
            return retainFailure(retain, schedule, firedAtMillis, HostEventHandoffStage.DELIVERY, error)
        }
        return HostEventHandoffOutcome.Delivered
    }

    private fun retainFailure(
        retain: (UndeliveredHostEventFire) -> Unit,
        schedule: AndroidHostEventSchedule,
        firedAtMillis: Long,
        stage: HostEventHandoffStage,
        error: Throwable,
    ): HostEventHandoffOutcome.Undelivered {
        val fire = UndeliveredHostEventFire(
            scheduleId = schedule.scheduleId,
            firedAtMillis = firedAtMillis,
            stage = stage,
            reason = error.toString(),
        )
        retain(fire)
        return HostEventHandoffOutcome.Undelivered(fire, error)
    }
}
