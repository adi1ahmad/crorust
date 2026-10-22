import sys
import time
import lldb

class AppleDebugSession:
    def __init__(self, debugger, connect_url, device_app_path=""):
        self.debugger = debugger
        self.connect_url = connect_url
        self.device_app_path = device_app_path
        self.listener = lldb.SBListener("crorust_session_listener")
        self.target = debugger.GetSelectedTarget()
        self.process = None

    def connect_and_run(self):
        self.listener.StartListeningForEventClass(
            self.debugger,
            lldb.SBProcess.GetBroadcasterClassName(),
            lldb.SBProcess.eBroadcastBitStateChanged |
            lldb.SBProcess.eBroadcastBitSTDOUT |
            lldb.SBProcess.eBroadcastBitSTDERR
        )

        error = lldb.SBError()
        self.process = self.target.ConnectRemote(self.listener, self.connect_url, None, error)
        if not error.Success():
            print(f"[crorust::debug] Remote connection failed: {error.GetCString()}", file=sys.stderr)
            return False

        if self.device_app_path:
            self.debugger.HandleCommand(f'target modules add "{self.device_app_path}"')

        if self.process.IsValid():
            self.process.Continue()
            return True
        return False

    def monitor_loop(self):
        if not self.process or not self.process.IsValid():
            return

        event = lldb.SBEvent()
        while self.process.IsValid():
            state = self.process.GetState()
            if state in (lldb.eStateExited, lldb.eStateCrashed, lldb.eStateDetached):
                break

            if self.listener.WaitForEvent(1, event):
                self._drain_stream(self.process.GetSTDOUT, sys.stdout)
                self._drain_stream(self.process.GetSTDERR, sys.stderr)

    def _drain_stream(self, reader, output_stream):
        while True:
            chunk = reader(4096)
            if not chunk:
                break
            output_stream.write(chunk)
            output_stream.flush()

    def detach_and_exit(self):
        if self.process and self.process.IsValid():
            self.process.Detach()
        self.debugger.HandleCommand("quit")
