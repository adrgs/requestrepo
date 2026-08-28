import { useState } from "react";
import {
  Modal,
  Input,
  Button,
  useOverlayState,
} from "@heroui/react";
import { Lock, AlertCircle, Eye, EyeOff } from "lucide-react";
import { useAuthStore } from "@/stores/authStore";

interface AdminAuthOverlayProps {
  onSubmit: (password: string) => Promise<void>;
}

export function AdminAuthOverlay({ onSubmit }: AdminAuthOverlayProps) {
  const [password, setPassword] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const authError = useAuthStore((s) => s.authError);
  const setAuthError = useAuthStore((s) => s.setAuthError);
  const { isOpen, open, close, setOpen, toggle } = useOverlayState({ defaultOpen: true });

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!password.trim() || isSubmitting) return;

    setIsSubmitting(true);
    setAuthError(null);

    try {
      await onSubmit(password);
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Modal state={{ isOpen, open, close, setOpen, toggle }}>
      <Modal.Backdrop isDismissable={false}>
        <Modal.Container>
          <Modal.Dialog>
            <form onSubmit={handleSubmit}>
              <Modal.Header className="flex flex-col gap-1">
                <div className="flex items-center gap-2">
                  <Lock className="h-5 w-5" />
                  <span>Authentication Required</span>
                </div>
              </Modal.Header>
              <Modal.Body>
                <p className="text-sm text-default-500 mb-4">
                  This RequestRepo instance requires an admin password to create
                  sessions.
                </p>

                <div className="relative">
                  <Input
                    type={showPassword ? "text" : "password"}
                    placeholder="Enter admin password"
                    value={password}
                    onChange={(e) => {
                      setPassword(e.target.value);
                      if (authError) setAuthError(null);
                    }}
                    autoFocus
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassword(!showPassword)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 focus:outline-hidden"
                  >
                    {showPassword ? (
                      <EyeOff className="h-4 w-4 text-default-400 hover:text-default-600" />
                    ) : (
                      <Eye className="h-4 w-4 text-default-400 hover:text-default-600" />
                    )}
                  </button>
                </div>

                {authError && (
                  <div className="flex items-center gap-2 text-danger text-sm mt-2">
                    <AlertCircle className="h-4 w-4" />
                    <span>{authError}</span>
                  </div>
                )}
              </Modal.Body>
              <Modal.Footer>
                <Button
                  type="submit"
                  variant="primary"
                  isDisabled={!password.trim()}
                >
                  {isSubmitting ? "Authenticating..." : "Authenticate"}
                </Button>
              </Modal.Footer>
            </form>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </Modal>
  );
}
