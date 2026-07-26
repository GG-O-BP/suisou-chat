import BackgroundTasks
import UIKit

private var continuedTask: BGContinuedProcessingTask?
private var activeRequestId: String?

@_cdecl("suisou_background_start")
func startBackgroundTask(
    _ requestIdPointer: UnsafePointer<CChar>,
    _ titlePointer: UnsafePointer<CChar>,
    _ subtitlePointer: UnsafePointer<CChar>
) -> Bool {
    guard #available(iOS 26.0, *) else {
        return false
    }
    let requestId = String(cString: requestIdPointer)
    let title = String(cString: titlePointer)
    let subtitle = String(cString: subtitlePointer)
    let taskIdentifier = "com.ggobp.suisou-chat.research.\(requestId)"
    activeRequestId = requestId
    let registered = BGTaskScheduler.shared.register(
        forTaskWithIdentifier: taskIdentifier,
        using: nil
    ) { task in
        guard let processingTask = task as? BGContinuedProcessingTask else {
            task.setTaskCompleted(success: false)
            return
        }
        continuedTask = processingTask
        processingTask.progress.totalUnitCount = 100
        processingTask.progress.completedUnitCount = 1
        processingTask.expirationHandler = {
            requestId.withCString { pointer in
                _ = suisou_cancel_research(pointer)
            }
            processingTask.setTaskCompleted(success: false)
            continuedTask = nil
            activeRequestId = nil
        }
    }
    guard registered else {
        activeRequestId = nil
        return false
    }
    let request = BGContinuedProcessingTaskRequest(
        identifier: taskIdentifier,
        title: title,
        subtitle: subtitle
    )
    request.strategy = .fail
    do {
        try BGTaskScheduler.shared.submit(request)
        return true
    } catch {
        activeRequestId = nil
        return false
    }
}

@_cdecl("suisou_background_update")
func updateBackgroundTask(
    _ requestIdPointer: UnsafePointer<CChar>,
    _ subtitlePointer: UnsafePointer<CChar>,
    _ completed: Double
) {
    guard #available(iOS 26.0, *) else {
        return
    }
    let requestId = String(cString: requestIdPointer)
    guard requestId == activeRequestId else {
        return
    }
    continuedTask?.progress.completedUnitCount = Int64(max(0.0, min(1.0, completed)) * 100.0)
    continuedTask?.updateTitle("Suisou 연구 잠수", subtitle: String(cString: subtitlePointer))
}

@_cdecl("suisou_background_stop")
func stopBackgroundTask(_ requestIdPointer: UnsafePointer<CChar>, _ succeeded: Bool) {
    guard #available(iOS 26.0, *) else {
        return
    }
    let requestId = String(cString: requestIdPointer)
    guard requestId == activeRequestId else {
        return
    }
    continuedTask?.setTaskCompleted(success: succeeded)
    continuedTask = nil
    activeRequestId = nil
}

@_silgen_name("suisou_cancel_research")
private func suisou_cancel_research(_ requestId: UnsafePointer<CChar>) -> Bool
