import init, {
  encode_into_bytes,
  HandshakeState,
  solve_pow,
} from "../parser/pkg/parser";
import { Render } from "./render/renderer";
import utils from "./utils";
import Player from "./objects/player";
import { decodePacket, initDecoder } from "./packetHandler";
import { ActionBar } from "./ui/actionBar";
import { WEAPONS } from "./data/weapons";
import { HitLeaf } from "./objects/leaf";
import GameObject from "./objects/object";

// Import assets
import LeafParticle from "./assets/particle_0.png";
import PlayerSprite from "./assets/head.png";
import Arm1Sprite from "./assets/arm_1.png";
import Arm2Sprite from "./assets/arm_2.png";
import Animal from "./objects/animal";
import WolfSprite from "./assets/wolf.png";
import FishSprite from "./assets/fish_1.png";
import { UpgradeBar } from "./ui/upgradeBar";
import HammerSprite from "./assets/weapons/hammer_1.png";
import BackPackSprite from "./assets/pack_0.png";

/**
 * Game class.
 */
export class Game {
  constructor() {
    this.renderer = new Render(this);

    this.players = [];
    this.animals = [];
    this.objects = [];
    this.inventory = [];
    this.leaves = [];

    this.actionBar = null;
    this.resourceDisplay = null; //new ResourceDisplay();
    this.upgradeBar = new UpgradeBar(this);

    this.my_player = null;

    this.moveInput = { up: false, down: false, left: false, right: false };
    this.lastMoveDir = null;
    this.zoomDelta = 0;

    this.lastAimDir = 0;

    this.chatBoxVisible = false;

    this.utils = { game: this };
    utils.bind(this.utils)();
  }

  /**
   * Initializes the game.
   */
  async init() {
    // Initialize WASM.
    await initDecoder();

    // Initialize renderer.
    await this.renderer.init();

    // Add event listener to the Enter Game button.
    document.getElementById("goButton").addEventListener("click", async () => {
      await this.enterGame();
    });

    // Get the challenge from the server and solve it to get the nonce.
    const challenge = await fetch(`http://127.0.0.1:3001/challenge`).then((r) =>
      r.text(),
    );
    // Solve the POW to get the nonce.
    const nonce = solve_pow(challenge, 4);

    // Create the WebTransport connection with the solved nonce and challenge.
    this.wt = new WebTransport(
      `https://127.0.0.1:6767/?pow=${nonce}&challenge=${challenge}`,
    );

    // Wait for the WebTransport connection to be ready.
    await this.wt.ready;
    // console.log("wt connected");

    // Initialize the datagram writer and reader.
    this.datagramWriter = this.wt.datagrams.writable.getWriter();
    this.datagramReader = this.wt.datagrams.readable.getReader();

    // console.log("starting handshake");

    // Send the ClientHello message over the datagram stream.
    const handshakeAndMsg = HandshakeState.create_client_hello();
    const clientHelloBytes = handshakeAndMsg.message();

    // Frame the ClientHello message with a 4-byte length prefix.
    const helloFramed = new ArrayBuffer(4 + clientHelloBytes.byteLength);
    new DataView(helloFramed).setUint32(0, clientHelloBytes.byteLength, false);
    new Uint8Array(helloFramed, 4).set(clientHelloBytes);

    // Create a bidirectional stream (reliable).
    const stream = await this.wt.createBidirectionalStream();

    // Initialize the stream writer and reader.
    this.streamWriter = stream.writable.getWriter();
    this.streamReader = stream.readable.getReader();

    // Send the ClientHello message over the stream.
    await this.streamWriter.write(new Uint8Array(helloFramed));
    // console.log("sent ClientHello");

    // Allocate a buffer to read the handshake response.
    let handshakeBuffer = new Uint8Array(0);

    // Read the handshake response from the stream.
    const readExact = async (n) => {
      while (handshakeBuffer.length < n) {
        const { value, done } = await this.streamReader.read();
        if (done) throw new Error("connection closed during handshake");
        const merged = new Uint8Array(handshakeBuffer.length + value.length);
        merged.set(handshakeBuffer);
        merged.set(value, handshakeBuffer.length);
        handshakeBuffer = merged;
      }
    };

    // Read the server's hello message length and data.
    await readExact(4);
    const serverHelloLen = new DataView(handshakeBuffer.buffer).getUint32(
      0,
      false,
    );
    // Read the server's hello message data.
    await readExact(4 + serverHelloLen);

    // Complete the handshake and initialize crypto.
    this.crypto = handshakeAndMsg.complete_handshake(
      handshakeBuffer.slice(4, 4 + serverHelloLen),
    );

    // console.log("handshake complete");
    // console.log(`send nonce: ${this.crypto.get_send_nonce()}`);
    // console.log(`receive nonce: ${this.crypto.get_recv_nonce()}`);

    // Wait for the handshake to complete before closing the stream.
    this.streamWriter.close().catch(() => {});
    this.streamReader.cancel().catch(() => {});

    // Add event listeners for user input.
    window.addEventListener("keydown", this.handleKey);
    window.addEventListener("keyup", this.handleKey);
    window.addEventListener("mousemove", this.handleMouseMove);
    window.addEventListener("mousedown", this.handleMousedown);
    window.addEventListener("mouseup", this.handleMousedown);

    // Start the read loops to receive packets from the server.
    this.read().catch((e) => console.error("read loop died:", e));

    // Start the reliable read loop to receive reliable streams from the server.
    this.readReliable().catch((e) =>
      console.error("reliable read loop died:", e),
    );
  }

  /**
   * Reads messages from the reliable stream. (TCP).
   */
  async readReliable() {
    // Read reliable streams from the server.
    const reader = this.wt.incomingUnidirectionalStreams.getReader();

    while (true) {
      // Wait for the next stream to be available.
      const { value: stream, done } = await reader.read();

      // If the stream is done, break the loop.
      if (done) break;

      // Handle the stream.
      this.handleReliableStream(stream).catch((e) =>
        console.error("reliable stream error:", e),
      );
    }
  }

  /**
   * Handles the reliable stream to the server.
   * @param {*} stream The reliable stream to handle.
   */
  async handleReliableStream(stream) {
    // Read the stream.
    const reader = stream.getReader();
    // Accumulate the stream data.
    let carry = new Uint8Array(0);

    // Read the stream data into the carry buffer.
    const readExact = async (n) => {
      const out = new Uint8Array(n);
      let offset = 0;

      // drain carry first
      if (carry.length > 0) {
        const take = Math.min(carry.length, n);
        out.set(carry.subarray(0, take), 0);
        carry = carry.subarray(take);
        offset += take;
      }

      while (offset < n) {
        const { value, done } = await reader.read();
        if (done) throw new Error("stream closed early");
        const need = n - offset;
        if (value.length <= need) {
          out.set(value, offset);
          offset += value.length;
        } else {
          out.set(value.subarray(0, need), offset);
          carry = value.subarray(need); // save the overshoot
          offset = n;
        }
      }
      return out;
    };

    // Read the payload length from the stream.
    const lenBuf = await readExact(4);
    const payloadLen = new DataView(lenBuf.buffer).getUint32(0, false);
    // Read the payload from the stream.
    const payload = await readExact(payloadLen);

    try {
      // Decrypt and decode the payload.
      const plaintext = this.crypto.decrypt(payload);
      const opcode = plaintext[0];
      if (opcode === 2 || opcode === 12) {
        const id = new DataView(
          plaintext.buffer,
          plaintext.byteOffset + 1,
          4,
        ).getUint32(0, true);
        const pkt = { code: opcode, data: { id } };
        this.handlePacket(pkt);
      } else {
        const packet = decodePacket(plaintext);
        this.handlePacket(packet);
      }

      // Handle the decoded packet.
    } catch (e) {
      console.error("reliable decrypt/decode error:", e);
    }
  }

  /**
   * Handle the chat box.
   */
  async handleChatBox() {
    // Get the chatbox element.
    const chatbox = document.getElementById("chatbox");

    // Toggle the chat box visibility.
    this.chatBoxVisible = !this.chatBoxVisible;

    // If the chat box is visible, display it and focus on the input.
    if (this.chatBoxVisible) {
      chatbox.style.display = "block";
      chatbox.focus();
      return;
    }

    // If the chat box is not visible, clear the input and hide the chat box.
    const message = chatbox.value.trim();

    // If the message is not empty, send it and clear the input.
    if (message.length > 0) {
      await this.sendEncrypted({ message }, 12);
    }

    // Clear the input and hide the chat box.
    chatbox.value = "";
    chatbox.style.display = "none";
    chatbox.blur();
  }

  /**
   * Handle key events.
   * @param {*} e KeyEvent.
   * @returns
   */
  handleKey = (e) => {
    const down = e.type === "keydown";
    const key = e.key.toLowerCase();

    // Handle enter key to send chat messages.
    if (key === "enter" && down) {
      e.preventDefault();

      (async () => {
        await this.handleChatBox();
      })();

      return;
    }

    // Ignore movement while typing.
    if (this.chatBoxVisible) return;

    // Handle movement keys.
    switch (key) {
      case "w":
      case "arrowup":
        this.moveInput.up = down;
        break;

      case "s":
      case "arrowdown":
        this.moveInput.down = down;
        break;

      case "a":
      case "arrowleft":
        this.moveInput.left = down;
        break;

      case "d":
      case "arrowright":
        this.moveInput.right = down;
        break;
    }
    // Send the input to the server.
    this.sendInput();
  };

  /**
   * Handle mouse down event.
   */
  handleMousedown = async (_) => {
    await this.sendEncrypted({}, 6);
  };

  /**
   * Handle mouse move event.
   * @param {*} e MouseEvent.
   */
  // TODO: Calculate aim based on player position and mouse position relative to the world.
  handleMouseMove = (e) => {
    // Calculate the center of the screen.
    const cx = window.innerWidth / 2;
    const cy = window.innerHeight / 2;

    // Calculate the aim direction based on mouse position relative to the center.
    const aim = Math.atan2(e.clientY - cy, e.clientX - cx);

    // Send the aim direction to the server if it has changed.
    if (this.lastAimDir === aim) return;
    this.lastAimDir = aim;

    // aim is sent piggy-backed on every position update from the server (case 3),
    // but also send immediately for responsiveness.
    this.sendAim(aim).catch(() => {});
  };

  /**
   *
   * @returns
   */
  sendInput = () => {
    // Early return if not connected or no player.
    if (!this.my_player) return;

    // Calculate the move direction based on input.
    let dir = null;
    if (
      this.moveInput.up ||
      this.moveInput.down ||
      this.moveInput.left ||
      this.moveInput.right
    ) {
      const dx = (this.moveInput.right ? 1 : 0) - (this.moveInput.left ? 1 : 0);
      const dy = (this.moveInput.down ? 1 : 0) - (this.moveInput.up ? 1 : 0);
      dir = Math.atan2(dy, dx);
    }

    if (this.lastMoveDir === dir) return;
    this.lastMoveDir = dir;

    // Send the move direction to the server.
    this.sendMove(dir).catch(() => {});
  };

  async enterGame() {
    const spawnData = { name: document.getElementById("nameInput").value };
    await this.sendEncrypted(spawnData, 1);
    console.log("spawn msg sent");
    document.getElementById("mainMenuContainer").remove();
    document.getElementById("darkener").remove();
  }

  async sendEncrypted(data, opcode) {
    if (!this.crypto || !this.datagramWriter) {
      throw new Error("cannot send - encryption not initialised");
    }
    const plaintext = encode_into_bytes(data, opcode);
    // crypto.encrypt() → Uint8Array of [ nonce(8) | ciphertext ]
    const packet = this.crypto.encrypt(plaintext);
    await this.datagramWriter.write(packet);
  }

  async read() {
    while (true) {
      const { value, done } = await this.datagramReader.read();
      if (done) break;
      if (!value || value.length < 9) continue; // 8 nonce + >=1 ciphertext byte

      try {
        const plaintext = this.crypto.decrypt(value);
        const opcode = plaintext[0];
        if (opcode === 2 || opcode === 12) {
          const id = new DataView(
            plaintext.buffer,
            plaintext.byteOffset + 1,
            4,
          ).getUint32(0, true);
          const pkt = { code: opcode, data: { id } };
          this.handlePacket(pkt);
        } else {
          const packet = decodePacket(plaintext);
          this.handlePacket(packet);
        }
      } catch (e) {
        console.error("decrypt/decode error:", e);
      }
    }
    console.warn("datagram reader closed");
  }

  handlePacket(packet) {
    switch (packet.code) {
      case 1: {
        console.log("player spawned:", packet.data);
        const { is_mine, data } = packet.data;
        const player = new Player(
          data,
          [
            this.renderer.textures[PlayerSprite],
            this.renderer.textures[Arm1Sprite],
            this.renderer.textures[Arm2Sprite],
            this.renderer.textures[BackPackSprite],
          ],
          this.renderer.world,
          this.utils,
        );

        player.setWeaponTexture(
          data.weapon === 1 ? this.renderer.textures[HammerSprite] : null,
        );

        this.players.push(player);

        if (is_mine) {
          this.my_player = player;
          this.actionBar = new ActionBar();
        }

        break;
      }

      case 3: {
        const { players } = packet.data;
        for (const p of players) {
          const player = this.utils.findPlayerByID(p.id);
          if (!player) continue;

          // console.error(p.weapon, player.weapon);

          // if (player.weapon !== p.weapon.id) {
          //   player.setWeaponTexture(
          //     p.weapon.id === 1 ? this.renderer.textures[HammerSprite] : null
          //   );
          // }

          player.x = p.x;
          player.y = p.y;
          player.lastAim = p.aim;
          player.aim = p.aim;
          player.sprite.x = p.x;
          player.sprite.y = p.y;
        }

        this.sendAim(this.lastAimDir).catch(() => {});
        break;
      }

      case 4: {
        const now = performance.now();

        // console.log(packet.data.animals);

        for (const a of this.animals) {
          a.sprite.visible = false;
        }

        for (const animalData of packet.data.animals) {
          let existing = this.utils.findAnimalByID(animalData.id);

          if (!existing) {
            let texture =
              animalData.animal_type === 1
                ? this.renderer.textures[FishSprite]
                : this.renderer.textures[WolfSprite];

            const newAnimal = new Animal(
              animalData,
              this.renderer.world,
              texture,
            );
            newAnimal.sprite.visible = true;

            this.animals.push(newAnimal);
          } else {
            existing.x = animalData.x;
            existing.y = animalData.y;
            existing.lastUpdate = now;
            existing.sprite.visible = true;
          }
        }

        for (let i = this.animals.length - 1; i >= 0; i--) {
          if (now - this.animals[i].lastUpdate > 3000) {
            this.animals[i].destroy();
            this.animals.splice(i, 1);
          }
        }
        break;
      }

      case 2: {
        const id = packet.data.id;
        const player = this.utils.findPlayerByID(id);
        if (player) {
          player.destroy();
          this.players = this.players.filter((p) => p.id !== id);
        }

        if (this.my_player && this.my_player.id === id) {
          this.cleanup();
          // window.location.reload();
        }
        break;
      }

      case 12: {
        const id = packet.data.id;
        if (this.my_player && this.my_player.id === id) {
          console.log("You died");
          this.cleanup();
          // window.location.reload();
        } else {
          const p = this.utils.findPlayerByID(id);
          if (p) {
            p.destroy();
            this.players = this.players.filter((pp) => pp.id !== id);
          }
        }
        break;
      }

      case 6: {
        console.log("hit event, entity:", packet.data.entity_id);
        const player = this.utils.findPlayerByID(packet.data.entity_id);
        if (player) {
          player.attackAnim = 1;
          player.punchTime = performance.now();
          player.animateRightArm = !player.animateRightArm;
        }
        break;
      }

      case 8: {
        // Update health packet
        const { id, new_health } = packet.data;
        const player = this.utils.findPlayerByID(id);
        if (player) {
          // Server currently only sends current health; assume max = 100 on client
          player.setHealth(new_health, 100);
        }
        break;
      }

      case 7:
        console.log("objects:", packet);

        this.objects.push(...packet.data.objects.map((x) => new GameObject(x)));
        break;

      case 9:
        this.inventory = packet.data.weapons.map(
          (x) => WEAPONS.find((y) => y.id == x) || {},
        );
        if (this.actionBar) this.actionBar.update(this.inventory);
        this.weapons = packet.data.weapons.map(
          (x) => WEAPONS.find((y) => y.id == x) || {},
        );

        this.my_player.weapon = packet.data.weapons;
        this.my_player.setWeaponTexture(
          packet.data.weapons[0] === 1
            ? this.renderer.textures[HammerSprite]
            : null,
        );

        console.error(
          packet.data.weapons[0] === 1
            ? this.renderer.textures[HammerSprite]
            : null,
        );

        break;

      case 10: {
        const object = this.utils.findObjectByID(packet.data.id);
        object.onHit(packet.data.dir);
        console.warn(packet.data);
        this.renderer.leaves.push(
          new HitLeaf(
            object.x + object.scale * Math.cos(packet.data.dir),
            object.y + object.scale * Math.sin(packet.data.dir),
            packet.data.dir,
            this.renderer.textures[LeafParticle],
            this.renderer.world,
          ),
        );
        break;
      }

      case 11: {
        const { wood, stone, food } = packet.data;
        // Set resources on local player if present (server sends SetResource to the player)
        if (this.my_player) {
          this.my_player.setResources(wood, stone, food);
        }
        console.log("resources updated:", wood, stone, food);
        break;
      }

      case 13: {
        game.utils
          .findPlayerByID(packet.data.id)
          .setChatMessage(packet.data.message);
        // game.utils.findPlayerByID(packet.data.id).timeSinceChat = performance.now();
        console.warn(packet.data, game.utils.findPlayerByID(packet.data.id));
        break;
      }

      case 14: {
        console.error(packet.data);
        this.upgradeBar.setWeapons(packet.data.upgrades.map((x) => WEAPONS[x]));
        this.upgradeBar.toggleVisibility(true);
        break;
      }

      default:
        console.warn("unknown packet type:", packet.code);
    }
  }

  /**
   * Send a move packet to the server.
   * @param {number} direction - The direction to move in.
   */
  async sendMove(direction) {
    await this.sendEncrypted({ dir: direction }, 2);
  }

  async chooseWeapon(weapon) {
    await this.sendEncrypted({ weapon: weapon }, 15);
  }

  /**
   * Send an aim packet to the server.
   * @param {number} direction - The direction to aim in.
   */
  async sendAim(direction) {
    await this.sendEncrypted({ dir: direction }, 5);
  }

  /**
   * Clean up resources.
   */
  cleanup() {
    // Remove all event listeners.
    window.removeEventListener("keydown", this.handleKey);
    window.removeEventListener("keyup", this.handleKey);
    window.removeEventListener("mousemove", this.handleMouseMove);
    window.removeEventListener("mousedown", this.handleMousedown);
    window.removeEventListener("mouseup", this.handleMousedown);
  }

  /**
   * Clean up resources.
   */
  cleanup() {
    // Remove all event listeners.
    window.removeEventListener("keydown", this.handleKey);
    window.removeEventListener("keyup", this.handleKey);
    window.removeEventListener("mousemove", this.handleMouseMove);
    window.removeEventListener("mousedown", this.handleMousedown);
    window.removeEventListener("mouseup", this.handleMousedown);

    // Release datagram writer and reader locks.
    if (this.datagramWriter) {
      this.datagramWriter.releaseLock();
      this.datagramWriter = null;
    }
    if (this.datagramReader) {
      this.datagramReader.cancel();
      this.datagramReader = null;
    }

    // Close WebTransport connection.
    if (this.wt) {
      this.wt.close();
      this.wt = null;
    }

    // Release crypto lock.
    this.crypto = null;
  }

  /**
   * Return whether we are connected.
   * @returns {boolean} Are we connected.
   */
  isConnected() {
    return this.crypto !== null && this.datagramWriter !== null;
  }

  /**
   * Return debug information about the connection.
   * @returns {Object | null} Debug information or null if not connected.
   */
  getDebugInfo() {
    if (!this.crypto) return null;

    return {
      sendNonce: this.crypto.get_send_nonce(),
      recvNonce: this.crypto.get_recv_nonce(),
      connected: this.isConnected(),
    };
  }
}

// Create and initialize the game.
const game = new Game();
await game.init();
