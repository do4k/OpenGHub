// Package prox implements the device plugin for the Logitech PRO X
// Wireless Gaming Headset.
//
// Unlike the G Pro Wireless mouse, this headset's USB dongle presents as
// a single ordinary HID++ device (driver hid-generic, one hidraw node),
// not a receiver multiplexing several paired peripherals over a separate
// DJ protocol layer — so it's addressed directly at device index 0xFF,
// with no interface- or device-index probing needed.
//
// Like internal/device/gpro, feature byte layouts here were verified
// against Solaar (github.com/pwr-Solaar/Solaar) rather than guessed.
package prox

import (
	"errors"
	"fmt"
	"log"
	"syscall"
	"time"

	"logitux/internal/device"
	"logitux/internal/hid"
	"logitux/internal/hidpp"
)

const vendorID = 0x046d
const productID uint16 = 0x0aba

const (
	featureSidetone  uint16 = 0x8300
	featureEqualizer uint16 = 0x8310
)

// probeTimeout bounds the initial liveness ping: the headset may be
// powered off or out of range, and failing fast keeps that from stalling
// discovery.
var probeTimeout = 300 * time.Millisecond

const operationTimeout = 2 * time.Second

func init() {
	device.Register(vendorID, []uint16{productID}, open)
}

// Headset is a device.Device for a connected PRO X Wireless.
type Headset struct {
	conn        *hidpp.Conn
	deviceIndex byte
	info        device.Info

	batteryFeatureIndex byte
	batteryKind         hidpp.BatteryKind

	sidetoneFeatureIndex byte

	eqFeatureIndex byte
	eqBands        []device.EqualizerBand
	eqMinDB        int
	eqMaxDB        int
}

func (h *Headset) Info() device.Info { return h.info }
func (h *Headset) Close() error      { return h.conn.Close() }

func open(backend hid.Backend, info hid.Info) (device.Device, error) {
	if info.ProductID != productID {
		return nil, fmt.Errorf("prox: unexpected product ID %04x", info.ProductID)
	}

	h, err := backend.Open(info)
	if err != nil {
		return nil, fmt.Errorf("prox: open %s: %w", info.Path, err)
	}
    conn := hidpp.Open(h)
	conn.SetTimeout(probeTimeout)

	// Safely retry Ping (max 2 attempts, 5s cooldown)
	var pingErr error
	for i := 0; i < 2; i++ {
		if _, _, pingErr = hidpp.Ping(conn, hidpp.DeviceIndexDirect); pingErr == nil {
			break
		}
		time.Sleep(5 * time.Second)
	}

	if pingErr != nil {
		// Ignore EPIPE (broken pipe) to avoid crashes
		if errors.Is(pingErr, syscall.EPIPE) {
			log.Printf("prox: ignoring EPIPE error for headset (dongle may be faulty)")
			return &Headset{
				info: device.Info{
					Name:      "PRO X Wireless Gaming Headset",
					Kind:      device.KindHeadset,
					Serial:    info.Serial,
					VendorID:  vendorID,
					ProductID: productID,
				},
				batteryKind: hidpp.BatteryKindNone,
			}, nil
		}
		conn.Close()
		return nil, fmt.Errorf("prox: ping headset: %w", pingErr)
	}
	conn.SetTimeout(operationTimeout)

	headset, err := buildHeadset(conn, info.Serial)
	if err != nil {
		conn.Close()
		return nil, err
	}
	return headset, nil
}

// buildHeadset resolves the HID++ features this plugin uses. Battery and
// the equalizer are treated as optional (see internal/device/gpro for the
// precedent and rationale); sidetone likewise, since there's no reason to
// assume every unit implements it either.
func buildHeadset(conn *hidpp.Conn, serial string) (*Headset, error) {
	batteryIdx, battKind, err := hidpp.ResolveBatteryFeature(conn, hidpp.DeviceIndexDirect)
	if err != nil {
		batteryIdx, battKind = 0, hidpp.BatteryKindNone
	}

	sidetoneIdx, ok, err := hidpp.GetFeatureIndex(conn, hidpp.DeviceIndexDirect, featureSidetone)
	if err != nil || !ok {
		sidetoneIdx = 0
	}

	h := &Headset{
		conn:        conn,
		deviceIndex: hidpp.DeviceIndexDirect,
		info: device.Info{
			Name:      "PRO X Wireless Gaming Headset",
			Kind:      device.KindHeadset,
			Serial:    serial,
			VendorID:  vendorID,
			ProductID: productID,
		},
		batteryFeatureIndex:  batteryIdx,
		batteryKind:          battKind,
		sidetoneFeatureIndex: sidetoneIdx,
	}

	if eqIdx, ok, err := hidpp.GetFeatureIndex(conn, hidpp.DeviceIndexDirect, featureEqualizer); err == nil && ok {
		bands, minDB, maxDB, err := discoverEqualizer(conn, eqIdx)
		if err == nil {
			h.eqFeatureIndex = eqIdx
			h.eqBands = bands
			h.eqMinDB = minDB
			h.eqMaxDB = maxDB
		}
	}

	return h, nil
}

// Battery implements device.BatteryStatus.
func (h *Headset) Battery() (percent int, charging bool, err error) {
	if h.batteryFeatureIndex == 0 {
		return 0, false, fmt.Errorf("prox: device has no supported battery feature")
	}
	return hidpp.ReadBattery(h.conn, h.deviceIndex, h.batteryFeatureIndex, h.batteryKind)
}
