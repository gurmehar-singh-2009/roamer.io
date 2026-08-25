import init, { decode_bytes, get_buffer_ptr, get_buffer_size } from "../parser/pkg/parser";

const DATA_OFFSET = 4;
let wasmMemory;
let bufferPtr;
let bufView; // single cached view

export const initDecoder = async () => {
  const wasm = await init();
  wasmMemory = wasm.memory;
  bufferPtr = get_buffer_ptr();
  const bufferSize = get_buffer_size();
  bufView = new Uint8Array(wasmMemory.buffer, bufferPtr, bufferSize);
};

export const decodePacket = (packetBytes) => {
  if (bufView.buffer !== wasmMemory.buffer) {
    bufView = new Uint8Array(wasmMemory.buffer, bufferPtr, get_buffer_size());
  }
  const len = packetBytes.length;
  bufView[0] =  len        & 0xff;
  bufView[1] = (len >>  8) & 0xff;
  bufView[2] = (len >> 16) & 0xff;
  bufView[3] = (len >> 24) & 0xff;
  bufView.set(packetBytes, DATA_OFFSET);
  return decode_bytes();
};