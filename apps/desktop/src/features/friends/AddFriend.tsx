import { useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { FileTrigger, Form } from "react-aria-components";
import { api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { TextField } from "../../ui/TextField";
import { FriendError } from "./FriendError";
import { readCanvas, readImageFile } from "./qr";
import { refreshFriends } from "./useFriends";

/** Ajout d'un ami : on colle son lien ou on lit son QR code (caméra ou image). */
export function AddFriendPanel() {
  const [text, setText] = useState("");
  const [camera, setCamera] = useState(false);
  const [scanError, setScanError] = useState<string | null>(null);
  const [sent, setSent] = useState(false);
  const preview = useMutation({ mutationFn: api.friends.preview });
  const send = useMutation({
    mutationFn: api.friends.sendRequest,
    onSuccess: () => {
      setText("");
      setSent(true);
      preview.reset();
      refreshFriends();
    },
  });

  const look = (raw: string) => {
    const v = raw.trim();
    setText(v);
    setSent(false);
    setScanError(null);
    send.reset();
    if (v) preview.mutate(v);
  };

  const fromImage = async (files: FileList | null) => {
    const file = files?.[0];
    if (!file) return;
    try {
      const found = await readImageFile(file);
      if (found) look(found);
      else setScanError(m.friends_scan_none());
    } catch {
      setScanError(m.friends_scan_none());
    }
  };

  const p = preview.data;

  return (
    <div className="panel">
      <h2 className="panel__title">{m.friends_add_title()}</h2>
      <p className="muted">{m.friends_add_body()}</p>
      <Form
        className="friends-add"
        onSubmit={(e) => {
          e.preventDefault();
          look(text);
        }}
      >
        <TextField
          label={m.friends_add_link()}
          autoComplete="off"
          spellCheck="false"
          value={text}
          onChange={(v) => {
            setText(v);
            preview.reset();
          }}
        />
        <div className="panel__actions">
          <Button type="submit" isDisabled={preview.isPending || text.trim().length === 0}>
            {preview.isPending ? m.working() : m.friends_add_check()}
          </Button>
          <Button variant="ghost" onPress={() => setCamera((c) => !c)}>
            {camera ? m.friends_scan_stop() : m.friends_scan_camera()}
          </Button>
          <FileTrigger
            acceptedFileTypes={["image/png", "image/jpeg", "image/webp"]}
            onSelect={(files) => void fromImage(files)}
          >
            <Button variant="ghost">{m.friends_scan_image()}</Button>
          </FileTrigger>
        </div>
      </Form>

      {camera && (
        <CameraScan
          onFound={(t) => {
            setCamera(false);
            look(t);
          }}
          onError={(msg) => {
            setCamera(false);
            setScanError(msg);
          }}
        />
      )}
      {scanError && (
        <p className="alert" role="alert">
          {scanError}
        </p>
      )}
      <FriendError error={preview.error} />

      {p && (
        <div className="friend-preview">
          <div className="identity">
            <Avatar name={p.name || "?"} src={p.avatar} size={48} />
            <div className="identity__text">
              <strong>{p.name || m.friends_unnamed()}</strong>
              <span className="muted">{m.friends_code({ code: p.code })}</span>
            </div>
          </div>
          {p.own ? (
            <p className="note">{m.friends_preview_own()}</p>
          ) : p.already ? (
            <p className="note">{m.friends_preview_already()}</p>
          ) : (
            <>
              <p className="muted">{m.friends_preview_hint()}</p>
              <div>
                <Button isDisabled={send.isPending} onPress={() => send.mutate(text)}>
                  {send.isPending ? m.working() : m.friends_send_request()}
                </Button>
              </div>
            </>
          )}
        </div>
      )}
      <FriendError error={send.error} />
      {sent && (
        <p className="note" role="status">
          {m.friends_request_sent()}
        </p>
      )}
    </div>
  );
}

/** Lecture du QR par la caméra, si le système l'autorise. */
function CameraScan({
  onFound,
  onError,
}: {
  onFound: (text: string) => void;
  onError: (message: string) => void;
}) {
  const video = useRef<HTMLVideoElement>(null);
  const found = useRef(onFound);
  const failed = useRef(onError);
  found.current = onFound;
  failed.current = onError;

  useEffect(() => {
    let stream: MediaStream | null = null;
    let timer = 0;
    let stopped = false;
    const canvas = document.createElement("canvas");

    const tick = () => {
      const v = video.current;
      if (stopped || !v) return;
      if (v.readyState >= 2 && v.videoWidth > 0) {
        const scale = Math.min(1, 800 / Math.max(v.videoWidth, v.videoHeight));
        canvas.width = Math.round(v.videoWidth * scale);
        canvas.height = Math.round(v.videoHeight * scale);
        canvas.getContext("2d")?.drawImage(v, 0, 0, canvas.width, canvas.height);
        const text = readCanvas(canvas);
        if (text) {
          found.current(text);
          return;
        }
      }
      timer = window.setTimeout(tick, 250);
    };

    if (!navigator.mediaDevices?.getUserMedia) {
      failed.current(m.friends_scan_no_camera());
      return;
    }
    navigator.mediaDevices
      .getUserMedia({ video: { facingMode: "environment" }, audio: false })
      .then((s) => {
        if (stopped) {
          for (const t of s.getTracks()) t.stop();
          return;
        }
        stream = s;
        const v = video.current;
        if (!v) return;
        v.srcObject = s;
        void v.play().then(tick, tick);
      })
      .catch(() => failed.current(m.friends_scan_no_camera()));

    return () => {
      stopped = true;
      window.clearTimeout(timer);
      for (const t of stream?.getTracks() ?? []) t.stop();
    };
  }, []);

  return (
    <div className="friends-camera">
      <video ref={video} muted playsInline className="friends-camera__video" />
      <p className="muted">{m.friends_scan_hint()}</p>
    </div>
  );
}
