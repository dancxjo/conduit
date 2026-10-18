import assert from "node:assert/strict";
import test from "node:test";
import { openBrowserTextEditor } from "../../targets/browser/host/assets/browser-text-editor.mjs";

function fixture(normalize = false) {
  class TextArea extends EventTarget {
    dataset = {}; text = "";
    get value() { return this.text; }
    set value(value) { this.text = normalize ? value.replace(/\r\n?/g,"\n") : value; }
    selectionStart = 0; selectionEnd = 0; selectionDirection = "none";
    setAttribute() {}
    setSelectionRange(start, end, direction) { this.selectionStart = start; this.selectionEnd = end; this.selectionDirection = direction; }
    remove() {}
  }
  let area;
  const window = new EventTarget();
  const container = {ownerDocument:{defaultView:window,createElement(tag) { assert.equal(tag,"textarea"); return area = new TextArea(); }}, replaceChildren() {}};
  const editor = openBrowserTextEditor(container), controller = new AbortController();
  const event = (type, properties = {}) => area.dispatchEvent(Object.assign(new Event(type), properties));
  return {editor, area, controller, event, window};
}
function decode(bytes) {
  const view = new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
  return {revision:view.getBigUint64(0,true),start:view.getUint32(8,true),end:view.getUint32(12,true),text:new TextDecoder().decode(bytes.subarray(16))};
}

test("DOM replacement uses scalar coordinates and returns to committed truth pending acknowledgment", async () => {
  const {editor,area,controller,event} = fixture();
  const pending = editor.next({revision:"9007199254740993",text:"a😀b",label:"Message"},controller.signal);
  area.value="aéb"; area.setSelectionRange(2,2,"none"); event("input");
  assert.deepEqual(decode(await pending), {revision:9007199254740993n,start:1,end:2,text:"é"});
  assert.equal(area.value,"a😀b"); assert.equal(area.readOnly,true);
  const next = editor.next({revision:"9007199254740994",text:"aéb"},controller.signal);
  assert.equal(area.selectionStart,2); assert.equal(area.readOnly,false);
  controller.abort(); await assert.rejects(next,/cancelled/); editor.close();
});

test("scripted composition is provisional and blur cancels its trailing events", async () => {
  const {editor,area,controller,event} = fixture();
  let completed=false;
  const next=editor.next({revision:"0",text:""},controller.signal).then(value => {completed=true; return value;});
  event("compositionstart"); area.value="café"; event("input",{isComposing:true});
  await Promise.resolve(); assert.equal(completed,false);
  event("blur"); event("compositionend"); event("input",{inputType:"insertCompositionText"});
  await Promise.resolve(); assert.equal(completed,false); assert.equal(area.value,"");
  event("beforeinput",{inputType:"insertText",isComposing:false});
  area.value="café"; event("input");
  assert.equal(decode(await next).text,"café"); editor.close();
});

test("invalid Unicode and UTF-8 capacity refusal preserve the pending request and committed value", async () => {
  const {editor,area,controller,event} = fixture();
  const next=editor.next({revision:"0",text:""},controller.signal);
  area.value="\ud800"; event("input");
  assert.equal(area.value,""); assert.equal(area.dataset.editDisposition,"invalid-unicode");
  area.value="é".repeat(129); event("input");
  assert.equal(area.value,""); assert.equal(area.dataset.editDisposition,"capacity-refused");
  area.value="café"; event("input"); assert.equal(decode(await next).text,"café"); editor.close();
});

test("a browser-normalized document refuses instead of inventing a second representation", async () => {
  const {editor,area,controller}=fixture(true);
  await assert.rejects(editor.next({revision:"0",text:"a\r\nb"},controller.signal),/UnsupportedDocumentRepresentation/);
  assert.equal(area.readOnly,true); assert.equal(area.value,""); editor.close();
});

test("window focus loss rejects pending editing and prevents a late edit from reviving the Play", async () => {
  const {editor,area,controller,event,window}=fixture();
  const next=editor.next({revision:"0",text:"kept"},controller.signal);
  event("compositionstart"); area.value="provisional";
  window.dispatchEvent(new Event("blur"));
  await assert.rejects(next,error => error.code === "FocusLost");
  assert.equal(area.value,"kept"); assert.equal(area.readOnly,true);
  event("compositionend"); event("input");
  await assert.rejects(editor.next({revision:"0",text:"kept"},controller.signal),error => error.code === "FocusLost");
  editor.close();
});
