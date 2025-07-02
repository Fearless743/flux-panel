package users

type UserModel struct {
	ID       int    `json:"id"`
	Username string `json:"username"`
	Password string `json:"password"`
	Email    string `json:"email"`
	IsActive bool   `json:"is_active"`
}

type LoginModel struct {
	Username string `json:"username"`
	Password string `json:"password"`
}
