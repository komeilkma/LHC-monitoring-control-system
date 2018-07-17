<?php 
include("includes/controller.php");
if($session->isAdmin()){
    header("Location: ".$configs->homePage());
    exit;
}
$form = new Form;
?>
<!DOCTYPE html>
<html>
    <head>
        <title>IPM Software</title>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">

        <link href="css/bootstrap.min.css" rel="stylesheet">
        <link href="fonts/Open Iconic/css/open-iconic-bootstrap.min.css" rel="stylesheet">

        <link href="css/style.css" rel="stylesheet">
        
    </head>

    <body>
        <!-- Pen Title-->
        <div class="pen-title">
            <h1></h1><span><a href=''></a></span>
            <img src="logo.png">
        </div>
        <!-- Form Module-->
        <div class="module form-module">
            <div class="toggle" id="link-login" style="display: none"><i class="oi oi-pencil"></i>Login
                <div class="tooltip in">Click Me</div>
            </div>
            <!-- Login -->
            <div class="form" id="form-login">
                <h2>Login to your account</h2>
                <form action="includes/process.php" method="POST">
                    <input type="text" name="username" placeholder="Username" value="<?php echo Form::value("username"); ?>"/>
                    <?php if(Form::error("username")) { echo "<div class='help-block' id='user-error'>".Form::error('username')."</div>"; } ?>
                    <input type="password" name="password" placeholder="Password" value="<?php echo Form::value("password"); ?>"/>
                    <?php if(Form::error("password")) { echo "<div class='help-block' id='pass-error'>".Form::error('password')."</div>"; } ?>
                    <button>Login</button>
                    <input type="hidden" name="form_submission" value="adminlogin">
                </form>
            </div>
            
            <!-- Reset Password -->
            <div class="form" id="form-reset" style="display: none">
                <?php
                if(isset($_SESSION['forgotpass'])){
                /* New password was generated for user and sent to user's email address. */
                    if($_SESSION['forgotpass']){
                        echo "<h1>New Password Generated</h1>";
                        echo "<p>Your new password has been generated and sent to the email address associated with your account.</p>";
                    /* Email could not be sent, therefore password was not edited in the database. */
                    } else{
                        echo "<h1>New Password Failure</h1>";
                        echo "<p>There was an error sending you the email with the new password, so your password has not been changed. Please contact Admin for more assistance or <a href='login.php'>try again</a>.</p>";
                    }
                    unset($_SESSION['forgotpass']);
                } else {
                unset($_SESSION['forgotpass']);
                ?>
                <h2>Reset Your Password</h2>
                <form action="../admin/includes/process.php" method="POST">
                    <input type="text" name="user"  placeholder="Username" value="<?php echo Form::value("user"); ?>"/>
                    <?php if(Form::error("user")) { echo "<div class='help-block' id='user-error'>".Form::error('user')."</div>"; } ?>
                    <input type="email" name="email" placeholder="Email Address" value="<?php echo Form::value("email"); ?>"/>
                    <?php if(Form::error("email")) { echo "<div class='help-block' id='user-error'>".Form::error('email')."</div>"; } ?>
                    <input type="hidden" name="form_submission" value="forgot_password">
                    <button>Reset Password</button>
                </form>
                <?php } ?>
            </div>
            
            <!-- Forgot Password Footer -->
            <div class="cta" id="link-reset"><a href="#reset">Forgot your password?</a></div>
        </div>
        <!-- END Form Module-->
        
        <!-- Footer -->
        <div class="text-muted text-center" id="login-footer">
            <small><span id="year-copy">2018</span> © <a href="" target="_blank">IPM</a></small>
        </div>

        <!-- JavaScript Resources -->
        <script src="js/jquery-2.1.3.min.js"></script>
        <script src="js/jquery-ui.js"></script>
        <script src="js/bootstrap.min.js"></script>
        <script src="js/admin-login.js"></script>
        
        <script>$(function(){ Login.init(); });</script>
        
        <script>
        // Initialize Tooltips
        $('[data-toggle="tooltip"], .show-tooltip').tooltip({container: 'body', animation: false});        
        </script>

    </body>
</html>
