import { PermissionsStatus } from "../api";

interface PermissionsModalProps {
  checking: boolean;
  status: PermissionsStatus | null;
  onClose: () => void;
  onRecheck: () => Promise<void>;
  onOpenAccessibility: () => void;
  onOpenAutomation: () => void;
  onRequestAccessibility: () => Promise<void>;
  onRequestAutomation: () => Promise<void>;
  closeOnBackdrop?: boolean;
}

export function PermissionsModal(props: PermissionsModalProps) {
  const status = props.status;
  const isMac = status?.platform === "macos";
  const canPaste = status?.can_paste ?? false;
  const closeOnBackdrop = props.closeOnBackdrop ?? true;

  return (
    <div 
      className={closeOnBackdrop ? "modalBackdrop" : "modalBackdrop modalBackdropStatic"}
      onClick={closeOnBackdrop ? props.onClose : undefined}
    >
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modalHeader">
          <div className="modalTitle">Finish setup</div>
          <button className="btn" onClick={props.onClose}>
            Skip
          </button>
        </div>

        <div className="section">
          <div className="hint">
            PowerPaste can copy immediately. To “paste into other apps” on double-click, macOS requires additional
            permissions.
          </div>
        </div>

        <div className="section">
          <div className="label">Status</div>
          <div className="hint">
            {props.checking
              ? "Checking permissions…"
              : canPaste
                ? "All set — paste is enabled."
                : isMac
                  ? "Paste is not enabled yet."
                  : "Paste automation is currently only supported on macOS."}
          </div>

          {status ? (
            <div className="hint">
              Accessibility: {status.accessibility_ok ? "OK" : "Missing"} • Automation: {status.automation_ok ? "OK" : "Missing"}
            </div>
          ) : null}

          {status?.details ? <div className="error">{status.details}</div> : null}
        </div>

        {isMac ? (
          <div className="section">
            <div className="label">Grant permissions</div>
            <div className="rowInline equalButtons">
              {!status?.accessibility_ok ? (
                <button className="btnPrimary" onClick={() => void props.onRequestAccessibility()}>
                  Request Accessibility
                </button>
              ) : (
                <button className="btn" disabled>
                  ✓ Accessibility
                </button>
              )}
              {!status?.automation_ok ? (
                <button className="btnPrimary" onClick={() => void props.onRequestAutomation()}>
                  Request Automation
                </button>
              ) : (
                <button className="btn" disabled>
                  ✓ Automation
                </button>
              )}
            </div>

            <div className="hint">
              Clicking each button will show a macOS system dialog. Click <strong>"Open System Preferences"</strong> when
              prompted, then <strong>enable the toggle</strong> for PowerPaste.
            </div>
            
            {status?.is_bundled === false ? (
              <>
                <div className="warningInline">
                  ⚠️ Running in <strong>development mode</strong>. The system dialog may not work for unsigned binaries.
                </div>
                <div className="hint" style={{ fontFamily: "monospace", fontSize: 11, wordBreak: "break-all" }}>
                  {status.executable_path}
                </div>
                <div className="hint">
                  If the prompt doesn't appear, manually add the binary path above in System Settings.
                </div>
                <div className="rowInline equalButtons">
                  <button className="btn" onClick={props.onOpenAccessibility}>
                    Open Accessibility Settings
                  </button>
                  <button className="btn" onClick={props.onOpenAutomation}>
                    Open Automation Settings
                  </button>
                </div>
              </>
            ) : null}
            <div className="hint">
              After granting permissions, click <strong>Re-check</strong> below to verify.
            </div>
          </div>
        ) : null}

        <div className="modalFooter">
          <button className="btn" disabled={props.checking} onClick={() => void props.onRecheck()}>
            Re-check
          </button>
          {canPaste ? (
            <button className="btnPrimary" disabled={props.checking} onClick={props.onClose}>
              Done
            </button>
          ) : (
            <button 
              className="btnPrimary" 
              disabled={props.checking} 
              onClick={async () => {
                if (!status?.accessibility_ok) {
                  await props.onRequestAccessibility();
                }
                if (!status?.automation_ok) {
                  // Small delay so prompts don't stack
                  await new Promise(r => setTimeout(r, 500));
                  await props.onRequestAutomation();
                }
                // Re-check after requesting
                await new Promise(r => setTimeout(r, 1000));
                await props.onRecheck();
              }}
            >
              Grant Permissions
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
