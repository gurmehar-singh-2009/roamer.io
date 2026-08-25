import { Assets, Container } from "pixi.js";
import { Leaf } from "../objects/leaf";

export class ResourceHandler {
    constructor() {
        this.resources = {
            wood: 0,
            stone: 0,
            food: 0,
            gold: 0,
        };

        this.woodIcon;
        this.resourceDisplay = new Container();
    }

    async loadIcons() {
        this.woodIcon = await Assets.load(Leaf);
    }

    renderUI() {

    }
}