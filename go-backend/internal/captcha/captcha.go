package captcha

import (
	"bytes"
	"encoding/base64"
	"fmt"
	"image"
	"image/color"
	"image/draw"
	"image/png"
	"math/rand"
	"strings"
	"sync"
	"time"

	"github.com/google/uuid"
)

const (
	ttl           = 5 * time.Minute
	secondaryTTL  = 5 * time.Minute
	defaultWidth  = 120
	defaultHeight = 40
)

type entry struct {
	answer    string
	expiresAt time.Time
	verified  bool // secondary ready for login
}

// Store 内存验证码（简化算术/字符）
type Store struct {
	mu   sync.Mutex
	data map[string]*entry
}

func NewStore() *Store {
	s := &Store{data: make(map[string]*entry)}
	go s.cleanupLoop()
	return s
}

var defaultStore = NewStore()

func Default() *Store { return defaultStore }

func (s *Store) cleanupLoop() {
	t := time.NewTicker(time.Minute)
	defer t.Stop()
	for range t.C {
		s.mu.Lock()
		now := time.Now()
		for k, v := range s.data {
			if now.After(v.expiresAt) {
				delete(s.data, k)
			}
		}
		s.mu.Unlock()
	}
}

type GenerateResult struct {
	ID            string `json:"id"`
	CaptchaID     string `json:"captchaId"`
	CaptchaImage  string `json:"captchaImage"` // data:image/png;base64,...
	Type          string `json:"type"`
	// 兼容部分前端字段
	BackgroundImage string `json:"backgroundImage,omitempty"`
}

// Generate 生成算术或字符验证码图
func (s *Store) Generate() (*GenerateResult, error) {
	var answer, display string
	typ := "ARITHMETIC"
	if rand.Intn(2) == 0 {
		a := rand.Intn(9) + 1
		b := rand.Intn(9) + 1
		op := []string{"+", "-", "×"}[rand.Intn(3)]
		switch op {
		case "+":
			answer = fmt.Sprintf("%d", a+b)
			display = fmt.Sprintf("%d+%d=?", a, b)
		case "-":
			if a < b {
				a, b = b, a
			}
			answer = fmt.Sprintf("%d", a-b)
			display = fmt.Sprintf("%d-%d=?", a, b)
		default:
			answer = fmt.Sprintf("%d", a*b)
			display = fmt.Sprintf("%d×%d=?", a, b)
		}
	} else {
		typ = "CHAR"
		const chars = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"
		var b strings.Builder
		for i := 0; i < 4; i++ {
			b.WriteByte(chars[rand.Intn(len(chars))])
		}
		answer = b.String()
		display = answer
	}

	imgB64, err := renderTextPNG(display)
	if err != nil {
		return nil, err
	}

	id := uuid.NewString()
	s.mu.Lock()
	s.data[id] = &entry{
		answer:    strings.ToLower(answer),
		expiresAt: time.Now().Add(ttl),
	}
	s.mu.Unlock()

	return &GenerateResult{
		ID:              id,
		CaptchaID:       id,
		CaptchaImage:    imgB64,
		BackgroundImage: imgB64,
		Type:            typ,
	}, nil
}

// Verify 校验用户输入，成功则标记 secondary 供登录使用
func (s *Store) Verify(id, code string) bool {
	if id == "" || code == "" {
		return false
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	e, ok := s.data[id]
	if !ok || time.Now().After(e.expiresAt) {
		delete(s.data, id)
		return false
	}
	if strings.ToLower(strings.TrimSpace(code)) != e.answer {
		return false
	}
	e.verified = true
	e.expiresAt = time.Now().Add(secondaryTTL)
	// 消费答案，防重放直接用同一 code
	e.answer = ""
	return true
}

// Secondary 登录二次校验（一次性）
func (s *Store) Secondary(id string) bool {
	if id == "" {
		return false
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	e, ok := s.data[id]
	if !ok || time.Now().After(e.expiresAt) || !e.verified {
		delete(s.data, id)
		return false
	}
	delete(s.data, id)
	return true
}

func renderTextPNG(text string) (string, error) {
	img := image.NewRGBA(image.Rect(0, 0, defaultWidth, defaultHeight))
	bg := color.RGBA{R: 245, G: 247, B: 250, A: 255}
	draw.Draw(img, img.Bounds(), &image.Uniform{C: bg}, image.Point{}, draw.Src)

	// 噪点
	for i := 0; i < 80; i++ {
		x, y := rand.Intn(defaultWidth), rand.Intn(defaultHeight)
		img.Set(x, y, color.RGBA{uint8(rand.Intn(200)), uint8(rand.Intn(200)), uint8(rand.Intn(200)), 255})
	}
	// 干扰线
	for i := 0; i < 3; i++ {
		c := color.RGBA{uint8(rand.Intn(180)), uint8(rand.Intn(180)), uint8(rand.Intn(180)), 255}
		x1, y1 := rand.Intn(defaultWidth), rand.Intn(defaultHeight)
		x2, y2 := rand.Intn(defaultWidth), rand.Intn(defaultHeight)
		drawLine(img, x1, y1, x2, y2, c)
	}

	// 简易 5x7 点阵字体绘制
	fg := color.RGBA{R: 30, G: 30, B: 30, A: 255}
	x := 10
	for _, ch := range text {
		glyph, ok := font5x7[ch]
		if !ok {
			glyph = font5x7['?']
		}
		drawGlyph(img, x, 8, glyph, fg)
		x += 14
	}

	var buf bytes.Buffer
	if err := png.Encode(&buf, img); err != nil {
		return "", err
	}
	return "data:image/png;base64," + base64.StdEncoding.EncodeToString(buf.Bytes()), nil
}

func drawLine(img *image.RGBA, x0, y0, x1, y1 int, c color.Color) {
	dx := abs(x1 - x0)
	dy := -abs(y1 - y0)
	sx, sy := 1, 1
	if x0 > x1 {
		sx = -1
	}
	if y0 > y1 {
		sy = -1
	}
	err := dx + dy
	for {
		img.Set(x0, y0, c)
		if x0 == x1 && y0 == y1 {
			break
		}
		e2 := 2 * err
		if e2 >= dy {
			err += dy
			x0 += sx
		}
		if e2 <= dx {
			err += dx
			y0 += sy
		}
	}
}

func abs(v int) int {
	if v < 0 {
		return -v
	}
	return v
}

func drawGlyph(img *image.RGBA, ox, oy int, rows [7]byte, c color.Color) {
	scale := 2
	for row := 0; row < 7; row++ {
		bits := rows[row]
		for col := 0; col < 5; col++ {
			if bits&(1<<uint(4-col)) != 0 {
				for dy := 0; dy < scale; dy++ {
					for dx := 0; dx < scale; dx++ {
						img.Set(ox+col*scale+dx, oy+row*scale+dy, c)
					}
				}
			}
		}
	}
}

// 极简 5x7 点阵
var font5x7 = map[rune][7]byte{
	'0': {0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E},
	'1': {0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E},
	'2': {0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F},
	'3': {0x0E, 0x11, 0x01, 0x06, 0x01, 0x11, 0x0E},
	'4': {0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02},
	'5': {0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E},
	'6': {0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E},
	'7': {0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08},
	'8': {0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E},
	'9': {0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C},
	'+': {0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00},
	'-': {0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00},
	'×': {0x00, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x00},
	'=': {0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00},
	'?': {0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04},
	'A': {0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11},
	'B': {0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E},
	'C': {0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E},
	'D': {0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E},
	'E': {0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F},
	'F': {0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10},
	'G': {0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F},
	'H': {0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11},
	'J': {0x01, 0x01, 0x01, 0x01, 0x11, 0x11, 0x0E},
	'K': {0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11},
	'L': {0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F},
	'M': {0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11},
	'N': {0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11},
	'P': {0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10},
	'Q': {0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D},
	'R': {0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11},
	'S': {0x0E, 0x11, 0x10, 0x0E, 0x01, 0x11, 0x0E},
	'T': {0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04},
	'U': {0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E},
	'V': {0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04},
	'W': {0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11},
	'X': {0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11},
	'Y': {0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04},
	'Z': {0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F},
}
