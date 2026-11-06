import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { keys, queryClient } from "../../lib/queries";
import { syncApi } from "./syncApi";
import { ForgotPassword } from "./ForgotPassword";
import { LinkForm } from "./LinkDevice";
import { WelcomePage } from "../onboarding/WelcomePage";
import { backupApi, RestorePanel } from "../backup/BackupSettings";

vi.mock("../../lib/api", async (orig) => ({ ...(await orig<typeof import("../../lib/api")>()), isDesktop: true }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => "C:\\Users\\li\\OneDrive\\DearUsBackup") }));

beforeEach(() => {
  queryClient.clear();
  queryClient.setQueryData(keys.appInfo, { version: "1.1.0", dataDir: "x", schemaVersion: 2, edition: "store" });
});
afterEach(() => {
  cleanup();
  queryClient.clear();
  vi.restoreAllMocks();
});
const wrap = (ui: React.ReactNode) => render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);

it("resets a forgotten password with the emailed code", async () => {
  const forgot = vi.spyOn(syncApi, "forgotPassword").mockResolvedValue();
  const reset = vi.spyOn(syncApi, "resetPassword").mockResolvedValue();
  wrap(<ForgotPassword initialEmail="li@example.com" />);
  fireEvent.click(screen.getByRole("button", { name: "Forgot your password?" }));
  fireEvent.click(screen.getByRole("button", { name: "Email me a code" }));
  await waitFor(() => expect(forgot).toHaveBeenCalledWith("li@example.com"));
  fireEvent.change(await screen.findByLabelText("Code from the email"), { target: { value: "12 34 56" } });
  fireEvent.change(screen.getByLabelText("New password"), { target: { value: "a new password" } });
  fireEvent.click(screen.getByRole("button", { name: "Save new password" }));
  await waitFor(() => expect(reset).toHaveBeenCalledWith("li@example.com", "123456", "a new password"));
});

it("links a new device with the code from the other one, as the same person", async () => {
  vi.spyOn(syncApi, "readDeviceLink").mockResolvedValue({ email: "li@example.com", fromDevice: "LI-DESKTOP" });
  const link = vi.spyOn(syncApi, "linkDevice").mockResolvedValue();
  wrap(<LinkForm />);
  fireEvent.change(screen.getByLabelText("Link code from your other device"), { target: { value: "DEARUS-LINK1-abc" } });
  expect(await screen.findByText("LI-DESKTOP")).toBeInTheDocument();
  expect(screen.getByText("li@example.com")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Your password"), { target: { value: "my password" } });
  fireEvent.click(screen.getByRole("button", { name: "Link this device" }));
  await waitFor(() => expect(link).toHaveBeenCalledWith("DEARUS-LINK1-abc", "my password"));
});

it("offers linking to another device on a fresh Store install", () => {
  wrap(<WelcomePage />);
  fireEvent.click(screen.getByRole("button", { name: "I already use Dear Us on another device" }));
  expect(screen.getByRole("heading", { name: "Link to your other device" })).toBeInTheDocument();
});

it("opens an encrypted backup with the recovery phrase", async () => {
  const inspect = vi
    .spyOn(backupApi, "inspect")
    .mockRejectedValueOnce("[backup_locked] This backup is encrypted. Enter your 24-word recovery phrase to open it.")
    .mockResolvedValueOnce({ createdAt: "2026-11-06T10:00:00Z", spaceName: "Our little world", entries: 12, files: 30, encrypted: true });
  wrap(<RestorePanel />);
  fireEvent.click(screen.getByRole("button", { name: "Choose backup folder" }));
  expect(await screen.findByText("This backup is encrypted")).toBeInTheDocument();
  const words = Array.from({ length: 24 }, (_, i) => `word${i}`).join(" ");
  fireEvent.change(screen.getByPlaceholderText("word1 word2 word3 …"), { target: { value: words } });
  fireEvent.click(screen.getByRole("button", { name: "Open backup" }));
  expect(await screen.findByText("Our little world")).toBeInTheDocument();
  expect(inspect).toHaveBeenLastCalledWith(expect.stringContaining("OneDrive"), words);
});
